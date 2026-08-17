//! Measured output-family evidence and closed-form output-parameter seeds.

use crate::modelfit::types::{OutputCurve, OutputParams, Polarity};
use crate::shared::numerics::{linear_fit_with_r2, median};

/// Extract the AOSTFT output-curve params (`alpha_sat`, `lambda`, `m`) from a set
/// of Id-Vd curves and the threshold `vt`. Each above-threshold curve contributes
/// one estimate; the per-parameter medians are returned. `None` when no curve has
/// a usable above-threshold region.
///
/// The extraction is closed-form per curve: `lambda` from the saturation-line
/// slope/intercept, `V_dsat` from the intersection of the linear tangent and the
/// saturation line (`alpha_sat = V_dsat / (Vg - VT)`), and `m` from the knee
/// ratio `Idsat_eff / Id(V_dsat) = 2^(1/m)`. The window fractions below are the
/// drift-sensitive choices that golden tests pin.
pub(in crate::modelfit) fn extract_output(
    curves: &[OutputCurve],
    vt: f64,
    polarity: Polarity,
) -> Option<OutputParams> {
    let s = polarity.sign();
    let mut alphas = Vec::new();
    let mut lambdas = Vec::new();
    let mut ms = Vec::new();
    for curve in curves {
        // Overdrive in the on-direction: s*(Vg - VT) (> 0 when the device is on),
        // so a p-channel curve (on at negative Vg) reads positive.
        let vov = s * (curve.vg - vt);
        if vov <= 0.0 {
            continue;
        }
        // Drain swept in the on-direction: flip Vds for p-channel so the
        // n-channel windowing (which needs Vd > 0) applies. Id is already |Id|.
        let vds_p;
        let vds: &[f64] = if polarity == Polarity::PChannel {
            vds_p = curve.vds.iter().map(|v| -v).collect::<Vec<_>>();
            &vds_p
        } else {
            &curve.vds
        };
        if let Some((alpha, lambda, m)) = extract_one_output(vds, &curve.id, vov) {
            alphas.push(alpha);
            lambdas.push(lambda);
            ms.push(m);
        }
    }
    Some(OutputParams {
        alpha_sat: median(&mut alphas)?,
        lambda: median(&mut lambdas)?,
        m: median(&mut ms)?,
    })
}

/// One clean measured saturation-tail slope used to qualify/refine output
/// conductance. Voltages are in the normalized on-frame.
pub(in crate::modelfit) struct OutputTailTarget {
    pub(in crate::modelfit) vgs: f64,
    pub(in crate::modelfit) vds: Vec<f64>,
    pub(in crate::modelfit) slope: f64,
}

/// Strong output curves whose final 40% is a clean positive straight line.
/// This excludes off curves and noisy tails before a derivative enters a fit.
pub(in crate::modelfit) fn output_tail_targets(
    curves: &[OutputCurve],
    polarity: Polarity,
) -> Vec<OutputTailTarget> {
    let family_peak = curves
        .iter()
        .flat_map(|curve| curve.id.iter().map(|id| id.abs()))
        .fold(0.0_f64, f64::max);
    let s = polarity.sign();
    let mut targets = Vec::new();
    for curve in curves {
        if curve.id.iter().map(|id| id.abs()).fold(0.0_f64, f64::max) < 0.2 * family_peak {
            continue;
        }
        let mut points: Vec<_> = curve
            .vds
            .iter()
            .zip(&curve.id)
            .filter_map(|(&vd, &id)| {
                let point = (s * vd, id.abs());
                (point.0 >= 0.0 && point.0.is_finite() && point.1.is_finite()).then_some(point)
            })
            .collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(vd_max) = points.last().map(|point| point.0) else {
            continue;
        };
        let tail: Vec<_> = points
            .into_iter()
            .filter(|point| point.0 >= 0.6 * vd_max)
            .collect();
        let vds: Vec<_> = tail.iter().map(|point| point.0).collect();
        let id: Vec<_> = tail.iter().map(|point| point.1).collect();
        let Some((slope, r2)) = output_tail_slope(&vds, &id) else {
            continue;
        };
        if slope > 0.0 && r2 >= 0.8 {
            targets.push(OutputTailTarget {
                vgs: polarity.map_vg(curve.vg),
                vds,
                slope,
            });
        }
    }
    targets
}

pub(in crate::modelfit) fn output_tail_slope(vds: &[f64], id: &[f64]) -> Option<(f64, f64)> {
    let points: Vec<_> = vds
        .iter()
        .zip(id)
        .filter(|(vd, current)| vd.is_finite() && current.is_finite())
        .collect();
    if points.len() < 3 {
        return None;
    }
    let count = points.len() as f64;
    let xmean = points.iter().map(|point| *point.0).sum::<f64>() / count;
    let ymean = points.iter().map(|point| *point.1).sum::<f64>() / count;
    let xx = points
        .iter()
        .map(|point| (*point.0 - xmean).powi(2))
        .sum::<f64>();
    let yy = points
        .iter()
        .map(|point| (*point.1 - ymean).powi(2))
        .sum::<f64>();
    if !(xx > 0.0 && yy > 0.0) {
        return None;
    }
    let slope = points
        .iter()
        .map(|point| (*point.0 - xmean) * (*point.1 - ymean))
        .sum::<f64>()
        / xx;
    let intercept = ymean - slope * xmean;
    let residual = points
        .iter()
        .map(|point| (*point.1 - (slope * *point.0 + intercept)).powi(2))
        .sum::<f64>();
    let r2 = 1.0 - residual / yy;
    (slope.is_finite() && r2.is_finite()).then_some((slope, r2))
}

/// Lower window fraction (of `Vd_max`) for the linear-region conductance fit.
const OUTPUT_LINEAR_FRACTION: f64 = 0.05;
/// Lower bound (fraction of `Vd_max`) of the saturation-line fit window. Kept
/// high so `Vd_eff` is flat (near `V_dsat`) and the fitted slope is the true
/// channel-length-modulation term, not residual knee curvature.
const OUTPUT_SATURATION_FRACTION: f64 = 0.75;

/// `(alpha_sat, lambda, m)` from a single output sub-sweep, or `None`. `vds`/`id`
/// are in the on-direction frame (Vds > 0, Id > 0); the caller flips a p-channel
/// sweep before calling.
fn extract_one_output(vds: &[f64], id: &[f64], vov: f64) -> Option<(f64, f64, f64)> {
    let n = vds.len().min(id.len());
    if n < 6 {
        return None;
    }
    // interp() below requires Vds ascending. An n-channel sweep already is, but a
    // p-channel sweep arrives descending (the caller negates its ascending negative
    // Vds), so sort the pair by Vds once here. The windowed fits are predicate-based
    // and order-independent, so this only fixes the interp precondition.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        vds[a]
            .partial_cmp(&vds[b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let vds_sorted: Vec<f64> = order.iter().map(|&i| vds[i]).collect();
    let id_sorted: Vec<f64> = order.iter().map(|&i| id[i]).collect();
    let vds = vds_sorted.as_slice();
    let id = id_sorted.as_slice();
    let vd_max = vds.iter().copied().fold(f64::MIN, f64::max);
    if vd_max <= 0.0 {
        return None;
    }

    // Linear-region conductance: slope of Id vs Vd over the low window.
    let lo_hi = OUTPUT_LINEAR_FRACTION * vd_max;
    let (lx, ly) = window(vds, id, |v| v > 0.0 && v <= lo_hi);
    if lx.len() < 2 {
        return None;
    }
    let (g_lin, _, _, _) = linear_fit_with_r2(&lx, &ly);

    // Saturation line: Id vs Vd over the upper window.
    let sat_lo = OUTPUT_SATURATION_FRACTION * vd_max;
    let (sx, sy) = window(vds, id, |v| v >= sat_lo);
    if sx.len() < 2 {
        return None;
    }
    let (s_sat, i0, _, _) = linear_fit_with_r2(&sx, &sy);
    if !g_lin.is_finite() || !s_sat.is_finite() || !i0.is_finite() {
        return None;
    }
    if g_lin <= s_sat || i0 <= 0.0 {
        return None;
    }

    // Saturation voltage = intersection of the linear tangent and saturation line.
    let vdsat = i0 / (g_lin - s_sat);
    if vdsat <= 0.0 {
        return None;
    }
    let alpha = vdsat / vov;
    let lambda = s_sat / i0;

    // Knee sharpness: Idsat_eff / Id(Vdsat) = 2^(1/m) -> m = ln2 / ln(ratio).
    let idsat_eff = i0 + s_sat * vdsat;
    let id_knee = interp(vds, id, vdsat)?;
    if id_knee <= 0.0 {
        return None;
    }
    let ratio = idsat_eff / id_knee;
    if ratio <= 1.0 {
        return None;
    }
    let m = std::f64::consts::LN_2 / ratio.ln();

    Some((alpha, lambda, m))
}

/// Collect `(x, y)` pairs whose `x` satisfies `pred`.
fn window(xs: &[f64], ys: &[f64], pred: impl Fn(f64) -> bool) -> (Vec<f64>, Vec<f64>) {
    let mut wx = Vec::new();
    let mut wy = Vec::new();
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        if pred(x) {
            wx.push(x);
            wy.push(y);
        }
    }
    (wx, wy)
}

/// Linear interpolation of `ys` at `x` over ascending `xs` (endpoint-clamped).
fn interp(xs: &[f64], ys: &[f64], x: f64) -> Option<f64> {
    let n = xs.len().min(ys.len());
    if n == 0 {
        return None;
    }
    if x <= xs[0] {
        return Some(ys[0]);
    }
    if x >= xs[n - 1] {
        return Some(ys[n - 1]);
    }
    for i in 1..n {
        if x <= xs[i] {
            let span = xs[i] - xs[i - 1];
            let t = if span > 0.0 {
                (x - xs[i - 1]) / span
            } else {
                0.0
            };
            return Some(ys[i - 1] + t * (ys[i] - ys[i - 1]));
        }
    }
    Some(ys[n - 1])
}

#[cfg(test)]
#[path = "../tests/output.rs"]
mod tests;
