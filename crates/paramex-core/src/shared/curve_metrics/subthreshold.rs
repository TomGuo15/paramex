//! Product-neutral subthreshold fitting over paired voltage/current slices.

use crate::shared::numerics::{linear_fit_with_r2, WindowedLinearFitter, FLOAT_EPSILON};
use crate::shared::numpy_compat::{argsort, banker_round, isclose, nanmedian, take_by};

pub(crate) const DEFAULT_SUBTHRESHOLD_DECADES_LADDER: [f64; 3] = [0.3, 0.1, 0.05];
pub(crate) const DEFAULT_SUBTHRESHOLD_R2_LADDER: [f64; 3] = [0.9, 0.85, 0.8];

/// Result of fitting `log10(|Id|)` against gate voltage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubthresholdFit {
    pub swing_mv_dec: f64,
    pub slope: f64,
    pub intercept: f64,
    pub r2: f64,
    pub points: usize,
}

pub(crate) fn fit_subthreshold(
    vg: &[f64],
    id_abs: &[f64],
    fit_range: Option<(f64, f64)>,
    min_points: usize,
) -> SubthresholdFit {
    let (slope, intercept, r2, points) = fit_log_window(vg, id_abs, fit_range);
    let swing_mv_dec = if points < min_points || !slope.is_finite() || slope.abs() <= FLOAT_EPSILON
    {
        f64::NAN
    } else {
        (1000.0 / slope).abs()
    };
    SubthresholdFit {
        swing_mv_dec,
        slope,
        intercept,
        r2,
        points,
    }
}

/// Fit the shared ParamEx automatic subthreshold window rule.
pub fn fit_subthreshold_auto(
    vg: &[f64],
    id_abs: &[f64],
    max_points: usize,
    min_decades: f64,
    min_points: usize,
    min_r2: f64,
    off_guard_decades: f64,
) -> SubthresholdFit {
    let fit_range = select_subthreshold_window(
        vg,
        id_abs,
        max_points,
        min_decades,
        min_points,
        min_r2,
        off_guard_decades,
    );
    fit_subthreshold(vg, id_abs, fit_range, min_points)
}

pub(crate) fn detect_noise_floor_log10(id_abs: &[f64], fraction: f64) -> Option<f64> {
    let mut log_id: Vec<f64> = id_abs
        .iter()
        .copied()
        .filter(|&value| value > 0.0 && value.is_finite())
        .map(f64::log10)
        .collect();
    if log_id.is_empty() {
        return None;
    }
    let bottom_count = (banker_round(log_id.len() as f64 * fraction) as usize)
        .max(1)
        .min(log_id.len());
    log_id.sort_by(f64::total_cmp);
    Some(nanmedian(&log_id[..bottom_count]))
}

fn sorted_log_xy(vg: &[f64], id_abs: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::new();
    let mut y = Vec::new();
    for (&voltage, &current) in vg.iter().zip(id_abs) {
        if voltage.is_finite() && current.is_finite() && current > 0.0 {
            x.push(voltage);
            y.push(current.log10());
        }
    }
    let order = argsort(&x);
    (take_by(&x, &order), take_by(&y, &order))
}

pub(crate) fn select_local_subthreshold_window(
    vg: &[f64],
    id_abs: &[f64],
    min_decades: f64,
    points: usize,
    min_r2: f64,
) -> Option<(f64, f64)> {
    let (xs, ys) = sorted_log_xy(vg, id_abs);
    if xs.len() < points.max(2) {
        return None;
    }
    let mut best: Option<(f64, f64, f64, f64)> = None;
    let mut start = 0;
    while start + points <= xs.len() {
        let end = start + points;
        let xx = &xs[start..end];
        let yy = &ys[start..end];
        let ymax = yy.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let ymin = yy.iter().copied().fold(f64::INFINITY, f64::min);
        if ymax - ymin < min_decades {
            start += 1;
            continue;
        }
        let (slope, _, r2, _) = linear_fit_with_r2(xx, yy);
        if !slope.is_finite() || !r2.is_finite() || r2 < min_r2 {
            start += 1;
            continue;
        }
        let score = slope.abs();
        let take = match best {
            None => true,
            Some((best_score, best_r2, _, _)) => {
                score > best_score || (isclose(score, best_score, 1e-5, 1e-8) && r2 > best_r2)
            }
        };
        if take {
            best = Some((score, r2, xx[0], xx[points - 1]));
        }
        start += 1;
    }
    best.map(|(_, _, lo, hi)| (lo, hi))
}

pub(crate) fn auto_select_subthreshold_window(
    vg: &[f64],
    id_abs: &[f64],
    max_points: usize,
    min_decades: f64,
    min_points: usize,
    min_r2: f64,
    off_guard_decades: f64,
) -> Option<(f64, f64)> {
    let (xs, ys) = sorted_log_xy(vg, id_abs);
    let total = xs.len();
    if total < min_points.max(2) {
        return None;
    }
    let global_y_min = ys.iter().copied().fold(f64::INFINITY, f64::min);

    let mut best_abs_slope = f64::NEG_INFINITY;
    let mut best_r2 = f64::NEG_INFINITY;
    let mut best_len = f64::NEG_INFINITY;
    let mut best_pair = None;

    for start in 0..total - 1 {
        let mut local_min = ys[start];
        let mut local_max = ys[start];
        let mut end = start;
        while end < total - 1 && end - start + 1 < max_points {
            end += 1;
            local_min = local_min.min(ys[end]);
            local_max = local_max.max(ys[end]);
            let length = end - start + 1;
            if length < min_points
                || local_max - local_min < min_decades
                || local_min < global_y_min + off_guard_decades
            {
                continue;
            }
            let (slope, _, r2, points) = linear_fit_with_r2(&xs[start..=end], &ys[start..=end]);
            if points < min_points
                || !slope.is_finite()
                || slope.abs() <= FLOAT_EPSILON
                || !r2.is_finite()
                || r2 < min_r2
            {
                continue;
            }
            let abs_slope = slope.abs();
            let length = length as f64;
            let take = abs_slope > best_abs_slope
                || (isclose(abs_slope, best_abs_slope, 1e-5, 1e-8)
                    && (r2 > best_r2 || (isclose(r2, best_r2, 1e-5, 1e-8) && length > best_len)));
            if take {
                best_abs_slope = abs_slope;
                best_r2 = r2;
                best_len = length;
                best_pair = Some((xs[start], xs[end]));
            }
        }
    }

    if best_pair.is_some() {
        return best_pair;
    }
    if off_guard_decades > 0.0 {
        return auto_select_subthreshold_window(
            vg,
            id_abs,
            max_points,
            min_decades,
            min_points,
            min_r2,
            0.0,
        );
    }
    None
}

pub(crate) fn select_subthreshold_window(
    vg: &[f64],
    id_abs: &[f64],
    max_points: usize,
    min_decades: f64,
    min_points: usize,
    min_r2: f64,
    off_guard_decades: f64,
) -> Option<(f64, f64)> {
    const NOISE_BUFFER_DEC: f64 = 0.5;
    const LOCAL_POINTS: usize = 5;

    let floor = detect_noise_floor_log10(id_abs, 0.25);
    let (masked_vg, masked_id): (Vec<f64>, Vec<f64>) = match floor {
        Some(floor) => {
            let threshold = floor + NOISE_BUFFER_DEC;
            vg.iter()
                .copied()
                .zip(id_abs.iter().copied())
                .filter(|(voltage, current)| {
                    voltage.is_finite()
                        && if *current > 0.0 {
                            current.log10()
                        } else {
                            f64::NEG_INFINITY
                        } > threshold
                })
                .unzip()
        }
        None => (vg.to_vec(), id_abs.to_vec()),
    };

    for (&decades, r2) in DEFAULT_SUBTHRESHOLD_DECADES_LADDER
        .iter()
        .zip(DEFAULT_SUBTHRESHOLD_R2_LADDER)
    {
        if let Some(window) =
            select_local_subthreshold_window(&masked_vg, &masked_id, decades, LOCAL_POINTS, r2)
        {
            return Some(window);
        }
    }

    auto_select_subthreshold_window(
        vg,
        id_abs,
        max_points,
        min_decades,
        min_points,
        min_r2,
        off_guard_decades,
    )
}

fn fit_log_window(
    vg: &[f64],
    id_abs: &[f64],
    fit_range: Option<(f64, f64)>,
) -> (f64, f64, f64, usize) {
    let samples = vg
        .iter()
        .copied()
        .zip(id_abs.iter().copied())
        .filter(|(voltage, current)| voltage.is_finite() && current.is_finite() && *current > 0.0)
        .map(|(voltage, current)| (voltage, current.abs().log10()));
    let fit = WindowedLinearFitter::new(samples).fit(fit_range);
    (fit.slope, fit.intercept, fit.r2, fit.points)
}
