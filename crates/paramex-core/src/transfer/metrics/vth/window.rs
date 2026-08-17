//! ELR V_TH window-selection policy.

use crate::shared::numerics::FLOAT_EPSILON;
use crate::shared::numpy_compat::isclose;
use crate::transfer::fit::{Transform, WindowedFitter};
use crate::transfer::types::SweepData;

/// The default ELR R2 ladder (`vth.py:440`): tried in order, skipping any below
/// `min_r2`.
pub(in crate::transfer) const DEFAULT_VT_R2_LADDER: [f64; 4] = [0.99, 0.97, 0.95, 0.90];

/// Choose the ELR VT fit window from highly linear candidates, preferring wider
/// windows over narrow top-end fits (`vth.py:357-430`).
///
/// Candidate windows are scored with the shared prefix-sum engine A
/// (`WindowedFitter::fit_indices`, O(1) per window) - the masking, sqrt
/// transform, and Vg-sort are identical to the fitter's, so the candidate point
/// sets match the legacy polyfit search (guarded by the lifted
/// `test_vt_window_equivalence`). Returns `None` when no candidate clears the
/// gates.
pub(in crate::transfer::metrics) fn auto_select_vt_window(
    vg: &[f64],
    id_abs: &[f64],
    window_size: usize,
    step: usize,
    min_points: usize,
    min_r2: f64,
) -> Option<(f64, f64)> {
    let fitter = WindowedFitter::new(
        &SweepData {
            vg: vg.to_vec(),
            id_abs: id_abs.to_vec(),
        },
        Transform::Sqrt,
    );
    let x = fitter.x();
    let n_total = x.len();
    if n_total < min_points.max(2) {
        return None;
    }

    let min_size = min_points.max(2);
    let preferred_min_size = min_points.max(window_size.min(n_total));
    let max_size = window_size.max(80).min(n_total);
    if max_size < min_size {
        return None;
    }
    let step = step.max(1);

    // (lo, hi, width, points, r2)
    let mut candidates: Vec<(f64, f64, f64, usize, f64)> = Vec::new();
    let collect_candidates =
        |first_size: usize, last_size: usize, candidates: &mut Vec<(f64, f64, f64, usize, f64)>| {
            let mut size = first_size;
            while size <= last_size {
                let mut start = 0usize;
                while start + size <= n_total {
                    let end = start + size;
                    let fit = fitter.fit_indices(start, end);
                    let bad_fit = fit.points < min_points
                        || !fit.slope.is_finite()
                        || fit.slope.abs() <= FLOAT_EPSILON;
                    if !bad_fit && fit.r2.is_finite() && fit.r2 >= min_r2 {
                        let lo = x[start];
                        let hi = x[end - 1];
                        candidates.push((lo, hi, hi - lo, fit.points, fit.r2));
                    }
                    start += step;
                }
                size += 5;
            }
        };

    collect_candidates(preferred_min_size, max_size, &mut candidates);
    if candidates.is_empty() && min_size < preferred_min_size {
        collect_candidates(min_size, preferred_min_size - 1, &mut candidates);
    }

    if candidates.is_empty() {
        return None;
    }

    let min_width = candidates.iter().map(|c| c.2).fold(f64::INFINITY, f64::min);
    let max_width = candidates
        .iter()
        .map(|c| c.2)
        .fold(f64::NEG_INFINITY, f64::max);
    let width_span = (max_width - min_width).max(FLOAT_EPSILON);

    let mut best_score = f64::NEG_INFINITY;
    let mut best_pair: Option<(f64, f64)> = None;
    for &(lo, hi, width, _n, r2) in &candidates {
        let width_norm = (width - min_width) / width_span;
        let score = r2 + 0.003 * width_norm;
        let tie_breaks_wider = match best_pair {
            None => true,
            Some((blo, bhi)) => width > (bhi - blo),
        };
        if score > best_score || (isclose(score, best_score, 1e-5, 1e-8) && tie_breaks_wider) {
            best_score = score;
            best_pair = Some((lo, hi));
        }
    }
    best_pair
}

/// ELR window selector with a progressive R2 ladder (`vth.py:433-463`). Tries
/// each `r2_ladder` threshold `>= min_r2` in order (falling back to `[min_r2]`
/// if none qualify) and returns the first window found.
pub(in crate::transfer) fn select_elr_vt_window(
    vg: &[f64],
    id_abs: &[f64],
    window_size: usize,
    step: usize,
    min_points: usize,
    min_r2: f64,
    r2_ladder: &[f64],
) -> Option<(f64, f64)> {
    let mut thresholds: Vec<f64> = r2_ladder.iter().copied().filter(|&r| r >= min_r2).collect();
    if thresholds.is_empty() {
        thresholds = vec![min_r2];
    }
    for threshold in thresholds {
        if let Some(win) =
            auto_select_vt_window(vg, id_abs, window_size, step, min_points, threshold)
        {
            return Some(win);
        }
    }
    None
}
