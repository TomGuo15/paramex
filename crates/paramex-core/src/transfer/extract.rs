//! Composition entry points (`extraction.pipeline`): run all per-sweep metrics
//! in one call.

mod directional;
mod primary;
mod sweep;
mod windows;

#[cfg(test)]
mod tests;

use crate::transfer::metrics::sweep::{has_backward_sweep, split_double_sweep};
use crate::transfer::types::{
    DeviceGeometry, ExpertRanges, ExtractionContext, ExtractionSettings, MetricResult, ParsedCurve,
    SweepData,
};

use directional::extract_directional_metrics;
use primary::extract_primary_metrics;
use windows::{auto_ss_window, auto_vt_window};

fn metric_value(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        f64::NAN
    }
}

/// Extract one display/export row for a parsed transfer curve
/// (`metrics_adapter.py:38-164`).
///
/// Splits the curve, picks the primary sweep (forward if a usable backward
/// exists, else the whole curve), auto-selects or honours pinned V_TH/SS
/// windows, runs V_TH/µ/SS/on-off on the primary, and — when a backward sweep
/// exists — re-extracts forward and backward with independently selected
/// windows. Total: degenerate input yields `NaN` metrics and `status="partial"`.
/// `geometry`/`expert_ranges` are explicit (the `Session` applies
/// the Python `None` defaults).
pub fn extract_metrics(
    curve: &ParsedCurve,
    settings: &ExtractionSettings,
    expert_ranges: &ExpertRanges,
    geometry: &DeviceGeometry,
) -> MetricResult {
    let context = ExtractionContext {
        cox_f_per_cm2: settings.cox_f_per_cm2(),
        aspect_ratio: geometry.aspect_ratio(),
    };
    let (forward, backward) = split_double_sweep(&curve.vg, &curve.id_abs);
    let has_backward = has_backward_sweep(&forward, &backward);
    let primary: SweepData = if has_backward {
        forward.clone()
    } else {
        SweepData {
            vg: curve.vg.clone(),
            id_abs: curve.id_abs.clone(),
        }
    };

    // Is the V_TH window user-pinned (manual) rather than auto-selected? Pinned
    // windows relax the fit gates so the chosen window is honoured as-is.
    let vt_pinned = expert_ranges.vt_range.is_some();

    let vt_window = expert_ranges
        .vt_range
        .or_else(|| auto_vt_window(&primary.vg, &primary.id_abs));
    let ss_window = expert_ranges
        .ss_range
        .or_else(|| auto_ss_window(&primary.vg, &primary.id_abs));

    let primary_metrics = extract_primary_metrics(
        &primary,
        &curve.id_abs,
        &forward,
        &backward,
        context,
        vt_window,
        ss_window,
        vt_pinned,
    );

    let directional_metrics = extract_directional_metrics(
        has_backward,
        &forward,
        &backward,
        context,
        expert_ranges,
        vt_window,
        ss_window,
        vt_pinned,
        &primary_metrics,
    );

    MetricResult {
        filename: curve.name.clone(),
        width_um: geometry.width_um,
        length_um: geometry.length_um,
        aspect_ratio: geometry.aspect_ratio(),
        geometry_source: geometry.source.clone(),
        vt: metric_value(primary_metrics.vt_result.vt),
        mu_sat: metric_value(primary_metrics.vt_result.mobility),
        ss_mv_dec: metric_value(primary_metrics.ss_result.ss_mv_dec),
        ion: metric_value(primary_metrics.ion),
        ioff: metric_value(primary_metrics.ioff),
        on_off_ratio: metric_value(primary_metrics.on_off_ratio),
        delta_vth_hysteresis: metric_value(primary_metrics.hysteresis),
        vt_window,
        ss_window,
        vt_window_bwd: directional_metrics.vt_window_bwd,
        ss_window_bwd: directional_metrics.ss_window_bwd,
        status: primary_metrics.status.to_string(),
        message: if primary_metrics.status == "ok" {
            String::new()
        } else {
            "Some metrics could not be extracted.".to_string()
        },
        has_backward_sweep: has_backward,
        vt_forward: metric_value(directional_metrics.forward.vt),
        mu_sat_forward: metric_value(directional_metrics.forward.mobility),
        ss_mv_dec_forward: metric_value(directional_metrics.forward.ss_mv_dec),
        ion_forward: metric_value(directional_metrics.forward.ion),
        ioff_forward: metric_value(directional_metrics.forward.ioff),
        on_off_ratio_forward: metric_value(directional_metrics.forward.on_off_ratio),
        vt_backward: metric_value(directional_metrics.backward.vt),
        mu_sat_backward: metric_value(directional_metrics.backward.mobility),
        ss_mv_dec_backward: metric_value(directional_metrics.backward.ss_mv_dec),
        ion_backward: metric_value(directional_metrics.backward.ion),
        ioff_backward: metric_value(directional_metrics.backward.ioff),
        on_off_ratio_backward: metric_value(directional_metrics.backward.on_off_ratio),
    }
}
