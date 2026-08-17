//! Overall-row composition, report sections, and plain-text cell formatting.

use super::format::{fmt, fmt_engineering_current, fmt_power_of_ten_text, format_count};
use super::schema::{
    column_by_key, column_keys_no_sweep, key_index, plain_label, results_to_rows, Cell, Formatter,
    COLUMNS,
};
use super::stats::{results_to_stats, StatRow};
use crate::transfer::types::MetricResult;

/// A finished report section: a title row, a header row (no-sweep plain labels),
/// and plain-string data rows. Consumed by the CSV writer (`transfer::report::csv`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ReportSection {
    pub(super) title: String,
    pub(super) header: Vec<String>,
    pub(super) rows: Vec<Vec<String>>,
}

/// Format one raw cell for display (`result_table_schema.py:371-386`
/// `format_cell`). `Text` passes through unchanged (so pre-formatted Overall
/// cells survive). For numeric columns `Null`/non-finite render the formatter's
/// NA sentinel.
pub(super) fn format_cell(key: &str, value: &Cell) -> String {
    if let Cell::Text(s) = value {
        return s.clone();
    }
    let opt = match value {
        Cell::Float(v) => Some(*v),
        Cell::Null | Cell::Text(_) => None,
    };
    match column_by_key(key).map(|c| c.formatter) {
        None | Some(Formatter::Text) => match opt {
            None => String::new(),
            // Unreachable in practice (text columns carry Text/Null), kept total.
            Some(v) => v.to_string(),
        },
        Some(Formatter::Current) => fmt_engineering_current(opt),
        Some(Formatter::PowerOfTen) => fmt_power_of_ten_text(opt),
        Some(Formatter::Number) => fmt(opt),
    }
}

/// How [`lookup_stat`] formats a statistic (and which sentinel a missing/`None`
/// value gets): integer count (`"0"`), engineering current, or fixed number
/// (both `"NA"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatFmt {
    /// Integer count: finite → `(v as i64)`, else `"0"`; None sentinel `"0"`.
    Count,
    /// Engineering-current format; None sentinel `"NA"`.
    Current,
    /// Fixed-number format; None sentinel `"NA"`.
    Number,
}

/// Look up one statistic and format it (`result_table_schema.py:257-277`
/// `_lookup_stat`). Missing row or `None`/non-finite value → `"0"`
/// ([`StatFmt::Count`]) or `"NA"`.
fn lookup_stat(
    stats: &[StatRow],
    scope: &str,
    metric: &str,
    statistic: &str,
    fmt_kind: StatFmt,
) -> String {
    let row = stats
        .iter()
        .find(|r| r.scope == scope && r.metric == metric);
    let value: Option<f64> = row.and_then(|r| match statistic {
        "count" => Some(r.count as f64),
        "mean" => r.mean,
        "std" => r.std,
        "min" => r.min,
        "median" => r.median,
        "max" => r.max,
        _ => None,
    });
    match value {
        None => match fmt_kind {
            StatFmt::Count => format_count(None),
            StatFmt::Current | StatFmt::Number => "NA".to_string(),
        },
        Some(v) => match fmt_kind {
            StatFmt::Count => format_count(Some(v)),
            StatFmt::Current => fmt_engineering_current(Some(v)),
            StatFmt::Number => fmt(Some(v)),
        },
    }
}

/// `"{mean} ± {std}"` (`result_table_schema.py:280-283` `_mean_std`). `±` is U+00B1.
fn mean_std(stats: &[StatRow], scope: &str, metric: &str, fmt_kind: StatFmt) -> String {
    let mean = lookup_stat(stats, scope, metric, "mean", fmt_kind);
    let std = lookup_stat(stats, scope, metric, "std", fmt_kind);
    format!("{} \u{00B1} {}", mean, std)
}

/// Build one "Overall (scope)" row as cells aligned to `COLUMNS`
/// (`result_table_schema.py:286-305` `_overall_row`). `log_label` is the plain
/// (`"log10"`) or html (`"log<sub>10</sub>"`) prefix for the on/off column.
pub(super) fn overall_row(stats: &[StatRow], scope: &str, log_label: &str) -> Vec<Cell> {
    let count = lookup_stat(stats, scope, "Vth", "count", StatFmt::Count);
    // One Cell per column, in COLUMNS order (a direct match avoids a map and any
    // key-borrow ambiguity).
    COLUMNS
        .iter()
        .map(|c| match c.key {
            "filename" => Cell::Text("Overall".to_string()),
            "sweep" => Cell::Text(format!("{} N={}", scope, count)),
            "W_um" | "L_um" | "W_over_L" => Cell::Float(f64::NAN),
            "geometry_source" | "status" => Cell::Text(String::new()),
            "Vth" => Cell::Text(mean_std(stats, scope, "Vth", StatFmt::Number)),
            "mu_sat" => Cell::Text(mean_std(stats, scope, "mu_sat", StatFmt::Number)),
            "SS_mV_dec" => Cell::Text(mean_std(stats, scope, "SS_mV_dec", StatFmt::Number)),
            "Ion" => Cell::Text(mean_std(stats, scope, "Ion", StatFmt::Current)),
            "Ioff" => Cell::Text(mean_std(stats, scope, "Ioff", StatFmt::Current)),
            "Ion_Ioff" => Cell::Text(format!(
                "{} {}",
                log_label,
                mean_std(stats, scope, "log10_Ion_Ioff", StatFmt::Number)
            )),
            "DeltaVth_hysteresis" => Cell::Text(mean_std(
                stats,
                "All",
                "DeltaVth_hysteresis",
                StatFmt::Number,
            )),
            "message" => Cell::Text("mean \u{00B1} std".to_string()),
            _ => Cell::Null,
        })
        .collect()
}

/// Forward/Backward sections for the human-readable CSV report
/// (`result_table_schema.py:403-413` `results_to_report_sections`). Empty when
/// there are no results. Each section masks the raw rows by sweep, appends its
/// scoped Overall row, formats every cell to plain text, and drops the Sweep
/// column.
pub(super) fn results_to_report_sections(results: &[MetricResult]) -> Vec<ReportSection> {
    if results.is_empty() {
        return Vec::new();
    }
    let raw_rows = results_to_rows(results);
    let stats = results_to_stats(results);
    let Some(sweep_idx) = key_index("sweep") else {
        return Vec::new();
    };
    let no_sweep = column_keys_no_sweep();
    // The no-sweep key → COLUMNS index map is identical for every row and section,
    // so resolve it once instead of re-scanning COLUMNS per cell.
    let no_sweep_idx: Vec<usize> = no_sweep.iter().filter_map(|k| key_index(k)).collect();
    let header: Vec<String> = no_sweep.iter().map(|k| plain_label(k)).collect();

    type SectionSpec = (&'static str, &'static str, fn(&str) -> bool);
    let specs: [SectionSpec; 2] = [
        ("Forward Results", "Forward", |s| {
            s == "Single" || s == "Forward"
        }),
        ("Backward Results", "Backward", |s| s == "Backward"),
    ];

    specs
        .iter()
        .map(|(title, scope, keep)| {
            let mut section_rows: Vec<Vec<Cell>> = raw_rows
                .iter()
                .filter(|row| match &row[sweep_idx] {
                    Cell::Text(s) => keep(s),
                    _ => false,
                })
                .cloned()
                .collect();
            section_rows.push(overall_row(&stats, scope, "log10"));
            let rows: Vec<Vec<String>> = section_rows
                .iter()
                .map(|row| {
                    no_sweep
                        .iter()
                        .zip(no_sweep_idx.iter())
                        .map(|(k, &idx)| format_cell(k, &row[idx]))
                        .collect()
                })
                .collect();
            ReportSection {
                title: title.to_string(),
                header: header.clone(),
                rows,
            }
        })
        .collect()
}
