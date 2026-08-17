//! ELR fit and mobility extraction for V_TH.

use crate::shared::numerics::FLOAT_EPSILON;
use crate::transfer::fit::{Transform, WindowedFitter};
use crate::transfer::types::{ExtractionContext, SweepData};

/// Linear fit of sqrt(Id) vs Vg used by the ELR V_TH method (`vth.py:24-33`).
/// `vt = -intercept/slope`. Float fields are `NaN` when the fit is rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::transfer) struct VTFitResult {
    pub(in crate::transfer) vt: f64,
    pub(in crate::transfer) mobility: f64,
    pub(in crate::transfer) slope: f64,
    pub(in crate::transfer) intercept: f64,
    pub(in crate::transfer) r2: f64,
    pub(in crate::transfer) points: usize,
}
/// Shared ELR fit on sqrt(|Id|) vs Vg without mobility (`vth.py:36-54`).
///
/// Python defaults: `min_points = 5`, `min_r2 = 0.995`. The fit is rejected
/// (NaN `vt`/`mobility`, fields carried through) when too few points, a
/// non-finite or ~zero slope, or (when `r2` is finite) `r2 < min_r2`.
fn fit_vt_only(
    vg: &[f64],
    id_abs: &[f64],
    fit_range: Option<(f64, f64)>,
    min_points: usize,
    min_r2: f64,
) -> VTFitResult {
    let fit = WindowedFitter::new(
        &SweepData {
            vg: vg.to_vec(),
            id_abs: id_abs.to_vec(),
        },
        Transform::Sqrt,
    )
    .fit(fit_range);
    // A rejected fit keeps the fit diagnostics (slope/intercept/r2/points) but
    // reports NaN vt/mobility - distinct from `nan_vt()`, which zeroes them.
    let reject = VTFitResult {
        vt: f64::NAN,
        mobility: f64::NAN,
        slope: fit.slope,
        intercept: fit.intercept,
        r2: fit.r2,
        points: fit.points,
    };
    if fit.points < min_points || !fit.slope.is_finite() || fit.slope.abs() <= FLOAT_EPSILON {
        return reject;
    }
    if fit.r2.is_finite() && fit.r2 < min_r2 {
        return reject;
    }
    let vt = -fit.intercept / fit.slope;
    VTFitResult {
        vt,
        mobility: f64::NAN,
        slope: fit.slope,
        intercept: fit.intercept,
        r2: fit.r2,
        points: fit.points,
    }
}

/// Extract V_TH using ELR on sqrt(|Id|) vs Vg (`vth.py:57-73`); delegates to
/// [`fit_vt_only`].
#[cfg(test)]
pub(in crate::transfer::metrics) fn extract_vth_elr(
    vg: &[f64],
    id_abs: &[f64],
    fit_range: Option<(f64, f64)>,
    min_points: usize,
    min_r2: f64,
) -> VTFitResult {
    fit_vt_only(vg, id_abs, fit_range, min_points, min_r2)
}

/// Extract V_TH and saturation mobility from the sqrt(|Id|) vs Vg fit
/// (`vth.py:316-354`). Mobility is `2*slope^2/(cox*aspect)` when both geometry
/// constants are `> 0`, else NaN; a rejected fit (NaN `vt`) is returned
/// unchanged.
pub(in crate::transfer) fn extract_vt_mu(
    vg: &[f64],
    id_abs: &[f64],
    context: ExtractionContext,
    fit_range: Option<(f64, f64)>,
    min_points: usize,
    min_r2: f64,
) -> VTFitResult {
    let fit = fit_vt_only(vg, id_abs, fit_range, min_points, min_r2);
    if !fit.vt.is_finite() {
        return fit;
    }
    let mobility = if context.cox_f_per_cm2 > 0.0 && context.aspect_ratio > 0.0 {
        2.0 * fit.slope * fit.slope / (context.cox_f_per_cm2 * context.aspect_ratio)
    } else {
        f64::NAN
    };
    VTFitResult { mobility, ..fit }
}
