//! Raw report-cell projection from `MetricResult`.

use crate::transfer::types::MetricResult;

use super::COLUMNS;

/// A report cell: a finite float, a blank/None, or a string (`pandas` object
/// cell). Pre-formatted "Overall" cells are `Text`; per-file numeric cells are
/// `Float`/`Null`.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::transfer::report) enum Cell {
    Float(f64),
    Null,
    Text(String),
}

/// `_export_float` (`result_table_schema.py:125-126`): finite -> `Float`, else `Null`.
pub(in crate::transfer::report) fn export_float(value: f64) -> Cell {
    if value.is_finite() {
        Cell::Float(value)
    } else {
        Cell::Null
    }
}

/// Per-(file, sweep) raw cell for `key` (`result_table_schema.py:99-122`
/// `_value_for_column` + `_SWEEP_ATTRS`). `sweep` is `"Forward"`/`"Backward"`/
/// `"Single"`.
pub(in crate::transfer::report) fn value_for_column(
    key: &str,
    result: &MetricResult,
    sweep: &str,
) -> Cell {
    match key {
        "filename" => Cell::Text(result.filename.clone()),
        "sweep" => Cell::Text(sweep.to_string()),
        "W_um" => export_float(result.width_um),
        "L_um" => export_float(result.length_um),
        "W_over_L" => export_float(result.aspect_ratio),
        "geometry_source" => Cell::Text(result.geometry_source.clone()),
        "DeltaVth_hysteresis" => export_float(result.delta_vth_hysteresis),
        "status" => Cell::Text(result.status.clone()),
        "message" => Cell::Text(result.message.clone()),
        "Vth" => export_float(sweep_pick(
            sweep,
            result.vt_forward,
            result.vt_backward,
            result.vt,
        )),
        "mu_sat" => export_float(sweep_pick(
            sweep,
            result.mu_sat_forward,
            result.mu_sat_backward,
            result.mu_sat,
        )),
        "SS_mV_dec" => export_float(sweep_pick(
            sweep,
            result.ss_mv_dec_forward,
            result.ss_mv_dec_backward,
            result.ss_mv_dec,
        )),
        "Ion" => export_float(sweep_pick(
            sweep,
            result.ion_forward,
            result.ion_backward,
            result.ion,
        )),
        "Ioff" => export_float(sweep_pick(
            sweep,
            result.ioff_forward,
            result.ioff_backward,
            result.ioff,
        )),
        "Ion_Ioff" => export_float(sweep_pick(
            sweep,
            result.on_off_ratio_forward,
            result.on_off_ratio_backward,
            result.on_off_ratio,
        )),
        _ => Cell::Null,
    }
}

/// Pick the forward/backward/single attribute (`_SWEEP_ATTRS` selection).
fn sweep_pick(sweep: &str, forward: f64, backward: f64, single: f64) -> f64 {
    match sweep {
        "Forward" => forward,
        "Backward" => backward,
        _ => single,
    }
}

/// One raw row per (file, sweep), cells aligned to `COLUMNS`
/// (`results_to_dataframe`, `result_table_schema.py:132-141`). Double-sweep
/// files emit `Forward` then `Backward`; single-sweep files emit one `Single`.
pub(in crate::transfer::report) fn results_to_rows(results: &[MetricResult]) -> Vec<Vec<Cell>> {
    let mut rows = Vec::new();
    for result in results {
        let sweeps: &[&str] = if result.has_backward_sweep {
            &["Forward", "Backward"]
        } else {
            &["Single"]
        };
        for &sweep in sweeps {
            rows.push(
                COLUMNS
                    .iter()
                    .map(|c| value_for_column(c.key, result, sweep))
                    .collect(),
            );
        }
    }
    rows
}
