//! Aggregate statistics (`result_table_schema.py` stats functions).

use crate::shared::numpy_compat::{nanmedian, std_sample};
use crate::transfer::types::MetricResult;

/// One long-format aggregate-stats row (`OVERALL_STATS_COLUMNS`). `count` is the
/// non-NaN sample size; the rest are `None` when undefined (empty column, or
/// `std` with count < 2, or a non-finite result via `_export_float`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct StatRow {
    pub(super) scope: String,
    pub(super) metric: String,
    pub(super) count: i64,
    pub(super) mean: Option<f64>,
    pub(super) std: Option<f64>,
    pub(super) min: Option<f64>,
    pub(super) median: Option<f64>,
    pub(super) max: Option<f64>,
}

const FILE_STAT_METRICS: &[&str] = &["W_um", "L_um", "W_over_L", "DeltaVth_hysteresis"];
const SWEEP_STAT_METRICS: &[&str] = &[
    "Vth",
    "mu_sat",
    "SS_mV_dec",
    "Ion",
    "Ioff",
    "log10_Ion",
    "log10_Ioff",
    "log10_Ion_Ioff",
];

/// `_export_float` semantics for a stat scalar: non-finite → `None`.
fn export_opt(v: f64) -> Option<f64> {
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

/// The `None`/undefined stat sentinel **after** the pandas round-trip.
///
/// Python's `_series_stat` returns `None` for an empty column and for `std`
/// with count < 2, but `results_to_stats_dataframe` materialises the rows into
/// a `pd.DataFrame` whose numeric stat columns are float64 (other rows carry
/// real floats). The DataFrame constructor coerces those `None`s to `NaN`, and
/// the consuming code reads back `NaN` via `to_dict("records")`. The reference is
/// therefore `NaN`, never JSON `null`, for these cells — so the port emits
/// `Some(NaN)` to match the actual downstream contract.
fn undefined_stat() -> Option<f64> {
    Some(f64::NAN)
}

/// `np.where(x > 0, log10(x), nan)` elementwise (`result_table_schema.py:216`).
fn log10_where_positive(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .map(|&v| if v > 0.0 { v.log10() } else { f64::NAN })
        .collect()
}

/// Compute one `StatRow` from a column that may contain NaN (dropna first, then
/// `_stat_row`/`_series_stat`).
fn stat_row(scope: &str, metric: &str, column: &[f64]) -> StatRow {
    let v: Vec<f64> = column.iter().copied().filter(|x| !x.is_nan()).collect();
    let count = v.len() as i64;
    let (mean, std, min, median, max) = if v.is_empty() {
        // Python `_series_stat` returns `None` for every stat of an empty
        // column; the DataFrame round-trip turns each into `NaN` (see
        // `undefined_stat`).
        (
            undefined_stat(),
            undefined_stat(),
            undefined_stat(),
            undefined_stat(),
            undefined_stat(),
        )
    } else {
        let n = v.len() as f64;
        let mean = export_opt(v.iter().sum::<f64>() / n);
        // `_series_stat` returns `None` for `std` when count < 2; the DataFrame
        // round-trip coerces that `None` to `NaN`.
        let std = if v.len() < 2 {
            undefined_stat()
        } else {
            export_opt(std_sample(&v))
        };
        let min = export_opt(v.iter().copied().fold(f64::INFINITY, f64::min));
        let max = export_opt(v.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let median = export_opt(nanmedian(&v));
        (mean, std, min, median, max)
    };
    StatRow {
        scope: scope.to_string(),
        metric: metric.to_string(),
        count,
        mean,
        std,
        min,
        median,
        max,
    }
}

/// `_export_float` over an array (`None` → NaN sentinel for the column math).
fn col_export(values: impl Iterator<Item = f64>) -> Vec<f64> {
    values
        .map(|v| if v.is_finite() { v } else { f64::NAN })
        .collect()
}

/// Long-format aggregate stats: scope "All" over file metrics, then "Forward"
/// and "Backward" over per-sweep metrics (`results_to_stats_dataframe`,
/// `result_table_schema.py:165-184`).
pub(super) fn results_to_stats(results: &[MetricResult]) -> Vec<StatRow> {
    let mut rows = Vec::new();

    // --- file scope ("All") ---
    let w_um = col_export(results.iter().map(|r| r.width_um));
    let l_um = col_export(results.iter().map(|r| r.length_um));
    let w_over_l = col_export(results.iter().map(|r| r.aspect_ratio));
    let dvth = col_export(results.iter().map(|r| r.delta_vth_hysteresis));
    for metric in FILE_STAT_METRICS {
        let column: &[f64] = match *metric {
            "W_um" => &w_um,
            "L_um" => &l_um,
            "W_over_L" => &w_over_l,
            "DeltaVth_hysteresis" => &dvth,
            _ => unreachable!(),
        };
        rows.push(stat_row("All", metric, column));
    }

    // --- forward scope (every result contributes its forward values) ---
    rows.extend(sweep_stat_rows("Forward", results));
    // --- backward scope (only dual-sweep results contribute) ---
    rows.extend(sweep_stat_rows("Backward", results));
    rows
}

/// Per-sweep stat rows for one scope (`_sweep_stat_dataframe` +
/// `_stat_rows`/`_stat_row`). Forward uses every result's `*_forward` values;
/// Backward uses only `has_backward_sweep` results' `*_backward` values.
fn sweep_stat_rows(scope: &str, results: &[MetricResult]) -> Vec<StatRow> {
    let forward = scope == "Forward";
    let selected: Vec<&MetricResult> = if forward {
        results.iter().collect()
    } else {
        results.iter().filter(|r| r.has_backward_sweep).collect()
    };

    let pick =
        |f: fn(&MetricResult) -> f64| -> Vec<f64> { col_export(selected.iter().map(|&r| f(r))) };
    // Pick the forward or backward field accessor once, per `scope`.
    let col = |fwd_fn: fn(&MetricResult) -> f64, bwd_fn: fn(&MetricResult) -> f64| -> Vec<f64> {
        if forward {
            pick(fwd_fn)
        } else {
            pick(bwd_fn)
        }
    };
    let vth = col(|r| r.vt_forward, |r| r.vt_backward);
    let mu = col(|r| r.mu_sat_forward, |r| r.mu_sat_backward);
    let ss = col(|r| r.ss_mv_dec_forward, |r| r.ss_mv_dec_backward);
    let ion = col(|r| r.ion_forward, |r| r.ion_backward);
    let ioff = col(|r| r.ioff_forward, |r| r.ioff_backward);
    let ratio = col(|r| r.on_off_ratio_forward, |r| r.on_off_ratio_backward);
    let log_ion = log10_where_positive(&ion);
    let log_ioff = log10_where_positive(&ioff);
    let log_ratio = log10_where_positive(&ratio);

    SWEEP_STAT_METRICS
        .iter()
        .map(|metric| {
            let column: &[f64] = match *metric {
                "Vth" => &vth,
                "mu_sat" => &mu,
                "SS_mV_dec" => &ss,
                "Ion" => &ion,
                "Ioff" => &ioff,
                "log10_Ion" => &log_ion,
                "log10_Ioff" => &log_ioff,
                "log10_Ion_Ioff" => &log_ratio,
                _ => unreachable!(),
            };
            stat_row(scope, metric, column)
        })
        .collect()
}
