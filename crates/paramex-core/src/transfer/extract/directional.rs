//! Forward/backward Transfer sweep policy for metric rows.

use crate::transfer::types::{ExpertRanges, ExtractionContext, SweepData, SweepExtractionResult};

use super::primary::PrimaryMetrics;
use super::sweep::{extract_single_sweep, NAN_SWEEP_METRICS};
use super::windows::{auto_ss_window, auto_vt_window};

/// V_TH fit R² gate for the per-direction forward/backward re-extraction in
/// `extract_metrics`: auto windows keep the default 0.995, pinned windows relax to 0.0.
const VT_SWEEP_R2_AUTO: f64 = 0.995;
const VT_SWEEP_R2_PINNED: f64 = 0.0;

pub(super) struct DirectionalMetrics {
    pub(super) forward: SweepExtractionResult,
    pub(super) backward: SweepExtractionResult,
    pub(super) vt_window_bwd: Option<(f64, f64)>,
    pub(super) ss_window_bwd: Option<(f64, f64)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_directional_metrics(
    has_backward: bool,
    forward: &SweepData,
    backward: &SweepData,
    context: ExtractionContext,
    expert_ranges: &ExpertRanges,
    vt_window: Option<(f64, f64)>,
    ss_window: Option<(f64, f64)>,
    vt_pinned: bool,
    primary: &PrimaryMetrics,
) -> DirectionalMetrics {
    if has_backward {
        let vt_window_bwd = expert_ranges
            .vt_range_bwd
            .or_else(|| auto_vt_window(&backward.vg, &backward.id_abs));
        let ss_window_bwd = expert_ranges
            .ss_range_bwd
            .or_else(|| auto_ss_window(&backward.vg, &backward.id_abs));
        let fwd = extract_single_sweep(
            forward,
            context,
            vt_window,
            ss_window,
            if vt_pinned {
                VT_SWEEP_R2_PINNED
            } else {
                VT_SWEEP_R2_AUTO
            },
        );
        let bwd = extract_single_sweep(
            backward,
            context,
            vt_window_bwd,
            ss_window_bwd,
            if expert_ranges.vt_range_bwd.is_some() {
                VT_SWEEP_R2_PINNED
            } else {
                VT_SWEEP_R2_AUTO
            },
        );
        return DirectionalMetrics {
            forward: fwd,
            backward: bwd,
            vt_window_bwd,
            ss_window_bwd,
        };
    }

    DirectionalMetrics {
        forward: SweepExtractionResult {
            vt: primary.vt_result.vt,
            mobility: primary.vt_result.mobility,
            ss_mv_dec: primary.ss_result.ss_mv_dec,
            ion: primary.ion,
            ioff: primary.ioff,
            on_off_ratio: primary.on_off_ratio,
        },
        backward: NAN_SWEEP_METRICS,
        vt_window_bwd: None,
        ss_window_bwd: None,
    }
}
