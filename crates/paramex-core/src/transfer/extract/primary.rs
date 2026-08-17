//! Primary-sweep Transfer metric policy.

use crate::shared::curve_metrics::on_off_ratio;
use crate::transfer::metrics::hysteresis::extract_delta_vth_hysteresis_curve_shift;
use crate::transfer::metrics::ss::{extract_ss, SSFitResult};
use crate::transfer::metrics::vth::{extract_vt_mu, VTFitResult};
use crate::transfer::types::{ExtractionContext, SweepData};

const HYST_TRIM_FRACTION: f64 = 0.2;
const HYST_MIN_POINTS: usize = 12;
const SS_EXTRACT_MIN_POINTS: usize = 5;

/// V_TH fit gates for the primary sweep in `extract_metrics`. An auto-selected
/// window demands a stricter fit (more points, higher R²); a user-pinned manual
/// window relaxes both so the chosen window is honoured as-is.
const VT_PRIMARY_MIN_POINTS_AUTO: usize = 10;
const VT_PRIMARY_MIN_POINTS_PINNED: usize = 5;
const VT_PRIMARY_R2_AUTO: f64 = 0.99;
const VT_PRIMARY_R2_PINNED: f64 = 0.0;

pub(super) struct PrimaryMetrics {
    pub(super) vt_result: VTFitResult,
    pub(super) ss_result: SSFitResult,
    pub(super) ion: f64,
    pub(super) ioff: f64,
    pub(super) on_off_ratio: f64,
    pub(super) hysteresis: f64,
    pub(super) status: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_primary_metrics(
    primary: &SweepData,
    full_id_abs: &[f64],
    forward: &SweepData,
    backward: &SweepData,
    context: ExtractionContext,
    vt_window: Option<(f64, f64)>,
    ss_window: Option<(f64, f64)>,
    vt_pinned: bool,
) -> PrimaryMetrics {
    let vt_result = extract_vt_mu(
        &primary.vg,
        &primary.id_abs,
        context,
        vt_window,
        if vt_pinned {
            VT_PRIMARY_MIN_POINTS_PINNED
        } else {
            VT_PRIMARY_MIN_POINTS_AUTO
        },
        if vt_pinned {
            VT_PRIMARY_R2_PINNED
        } else {
            VT_PRIMARY_R2_AUTO
        },
    );
    let ss_result = extract_ss(
        &primary.vg,
        &primary.id_abs,
        ss_window,
        SS_EXTRACT_MIN_POINTS,
    );
    let (ion, ioff, on_off_ratio) = on_off_ratio(full_id_abs);
    // Reuse the forward/backward split rather than re-splitting the same curve:
    // `_auto` is just `split_double_sweep` plus this curve-shift call.
    let hysteresis = extract_delta_vth_hysteresis_curve_shift(
        forward,
        backward,
        HYST_TRIM_FRACTION,
        HYST_MIN_POINTS,
    );
    let status = if vt_result.vt.is_finite() && ss_result.ss_mv_dec.is_finite() {
        "ok"
    } else {
        "partial"
    };
    PrimaryMetrics {
        vt_result,
        ss_result,
        ion,
        ioff,
        on_off_ratio,
        hysteresis,
        status,
    }
}
