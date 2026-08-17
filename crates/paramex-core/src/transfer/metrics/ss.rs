//! Subthreshold-swing fit, extraction, and window selection (`extraction.ss`).

use crate::shared::curve_metrics::{fit_subthreshold, SubthresholdFit};

pub(in crate::transfer) use crate::shared::curve_metrics::select_subthreshold_window as select_ss_window;

/// Linear fit of log10(|Id|) vs Vg in the subthreshold region; `SS = 1000/slope`
/// mV/dec (`ss.py:17-25`). Float fields are `NaN` when the fit is rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::transfer) struct SSFitResult {
    pub(in crate::transfer) ss_mv_dec: f64,
    pub(in crate::transfer) slope: f64,
    pub(in crate::transfer) intercept: f64,
    pub(in crate::transfer) r2: f64,
    pub(in crate::transfer) points: usize,
}

/// Extract subthreshold swing from an engine-A fit on log10(|Id|) vs Vg
/// (`ss.py:28-51`). Python default `min_points = 5`. `SS = |1000/slope|`; NaN
/// when too few points or a non-finite / ~zero slope.
pub(in crate::transfer) fn extract_ss(
    vg: &[f64],
    id_abs: &[f64],
    fit_range: Option<(f64, f64)>,
    min_points: usize,
) -> SSFitResult {
    into_transfer_result(fit_subthreshold(vg, id_abs, fit_range, min_points))
}

fn into_transfer_result(fit: SubthresholdFit) -> SSFitResult {
    SSFitResult {
        ss_mv_dec: fit.swing_mv_dec,
        slope: fit.slope,
        intercept: fit.intercept,
        r2: fit.r2,
        points: fit.points,
    }
}
