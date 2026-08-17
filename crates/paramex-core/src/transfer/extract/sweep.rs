//! Per-sweep Transfer extraction primitives.

use crate::shared::curve_metrics::on_off_ratio;
use crate::transfer::metrics::ss::extract_ss;
use crate::transfer::metrics::vth::extract_vt_mu;
use crate::transfer::types::{ExtractionContext, SweepData, SweepExtractionResult};

const SWEEP_EXTRACT_MIN_POINTS: usize = 5;

pub(super) const NAN_SWEEP_METRICS: SweepExtractionResult = SweepExtractionResult {
    vt: f64::NAN,
    mobility: f64::NAN,
    ss_mv_dec: f64::NAN,
    ion: f64::NAN,
    ioff: f64::NAN,
    on_off_ratio: f64::NAN,
};

/// Run V_TH / mobility / SS / Ion-Ioff extraction on one sweep with the given
/// windows (`pipeline.py:20-43`). `min_r2` is forwarded to [`extract_vt_mu`]
/// (`extract_metrics` lowers it to 0.0 for a user-pinned manual window).
pub(super) fn extract_single_sweep(
    sweep: &SweepData,
    context: ExtractionContext,
    vt_range: Option<(f64, f64)>,
    ss_range: Option<(f64, f64)>,
    min_r2: f64,
) -> SweepExtractionResult {
    let vt_res = extract_vt_mu(
        &sweep.vg,
        &sweep.id_abs,
        context,
        vt_range,
        SWEEP_EXTRACT_MIN_POINTS,
        min_r2,
    );
    let ss_res = extract_ss(&sweep.vg, &sweep.id_abs, ss_range, SWEEP_EXTRACT_MIN_POINTS);
    let (ion, ioff, on_off_ratio) = on_off_ratio(&sweep.id_abs);
    SweepExtractionResult {
        vt: vt_res.vt,
        mobility: vt_res.mobility,
        ss_mv_dec: ss_res.ss_mv_dec,
        ion,
        ioff,
        on_off_ratio,
    }
}
