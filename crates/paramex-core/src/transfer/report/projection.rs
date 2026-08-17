//! Typed application projection for the Transfer results table.
//!
//! Report serialization keeps its byte-exact formatting implementation, while
//! this projection exposes domain values for callers that own presentation.

use super::schema::{value_for_column, Cell};
use super::stats::{results_to_stats, StatRow};
use crate::transfer::types::MetricResult;

/// Canonical Transfer result columns, in report order.
///
/// The enum is the stable column identity. Callers may choose a narrower
/// display order without depending on report-schema implementation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum ResultsTableColumn {
    Filename,
    Sweep,
    WidthUm,
    LengthUm,
    AspectRatio,
    GeometrySource,
    ThresholdVoltage,
    SaturationMobility,
    SubthresholdSwing,
    OnCurrent,
    OffCurrent,
    OnOffRatio,
    ThresholdHysteresis,
    Status,
    Message,
}

impl ResultsTableColumn {
    /// Every canonical column in report order.
    pub const ALL: [Self; 15] = [
        Self::Filename,
        Self::Sweep,
        Self::WidthUm,
        Self::LengthUm,
        Self::AspectRatio,
        Self::GeometrySource,
        Self::ThresholdVoltage,
        Self::SaturationMobility,
        Self::SubthresholdSwing,
        Self::OnCurrent,
        Self::OffCurrent,
        Self::OnOffRatio,
        Self::ThresholdHysteresis,
        Self::Status,
        Self::Message,
    ];

    /// Stable report key shared with canonical CSV serialization.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Filename => "filename",
            Self::Sweep => "sweep",
            Self::WidthUm => "W_um",
            Self::LengthUm => "L_um",
            Self::AspectRatio => "W_over_L",
            Self::GeometrySource => "geometry_source",
            Self::ThresholdVoltage => "Vth",
            Self::SaturationMobility => "mu_sat",
            Self::SubthresholdSwing => "SS_mV_dec",
            Self::OnCurrent => "Ion",
            Self::OffCurrent => "Ioff",
            Self::OnOffRatio => "Ion_Ioff",
            Self::ThresholdHysteresis => "DeltaVth_hysteresis",
            Self::Status => "status",
            Self::Message => "message",
        }
    }

    /// Index of this column in [`Self::ALL`] and every projected row.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Whether ordinary measurement cells in this column are numeric.
    pub const fn is_numeric(self) -> bool {
        !matches!(
            self,
            Self::Filename | Self::Sweep | Self::GeometrySource | Self::Status | Self::Message
        )
    }

    /// Whether numeric measurement cells represent current in amperes.
    pub const fn is_current(self) -> bool {
        matches!(self, Self::OnCurrent | Self::OffCurrent)
    }

    /// Whether numeric measurement cells represent the unitless on/off ratio.
    pub const fn is_ratio(self) -> bool {
        matches!(self, Self::OnOffRatio)
    }

    /// Whether the value can differ between forward and backward sweeps.
    pub const fn is_sweep_aware(self) -> bool {
        matches!(
            self,
            Self::Sweep
                | Self::ThresholdVoltage
                | Self::SaturationMobility
                | Self::SubthresholdSwing
                | Self::OnCurrent
                | Self::OffCurrent
                | Self::OnOffRatio
        )
    }
}

/// Sweep represented by one measurement or aggregate row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultsTableSweep {
    Single,
    Forward,
    Backward,
}

impl ResultsTableSweep {
    const fn report_name(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Forward => "Forward",
            Self::Backward => "Backward",
        }
    }
}

/// One typed result-table cell.
///
/// `Number` is always finite. Summary options contain only finite values;
/// undefined means and sample standard deviations are `None`.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultsTableCell {
    Missing,
    Text(String),
    Number(f64),
    Sweep(ResultsTableSweep),
    /// Canonical marker for the aggregate rows' filename cell.
    Overall,
    /// Mean and sample standard deviation in the column's native units.
    Summary {
        mean: Option<f64>,
        sample_std_dev: Option<f64>,
    },
    /// Mean and sample standard deviation after a base-10 logarithm.
    Log10Summary {
        mean: Option<f64>,
        sample_std_dev: Option<f64>,
    },
    /// Canonical marker for the aggregate rows' explanatory message cell.
    SummaryCaption,
}

/// Whether a projected row is a measured sweep or an aggregate summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultsTableRowKind {
    Measurement,
    /// Aggregate count for the row's sweep scope.
    Overall {
        count: usize,
    },
}

/// One canonical typed row.
///
/// `cells` is aligned with [`ResultsTableColumn::ALL`]. Consecutive rows that
/// share a filename (including the two overall rows) share `group_span`;
/// `group_position == 0` identifies the leader.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultsTableRow {
    pub cells: Vec<ResultsTableCell>,
    pub sweep: ResultsTableSweep,
    pub kind: ResultsTableRowKind,
    pub group_position: usize,
    pub group_span: usize,
}

/// Session-owned snapshot of canonical Transfer results-table semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultsTableProjection {
    pub columns: &'static [ResultsTableColumn],
    pub rows: Vec<ResultsTableRow>,
}

fn typed_cell(cell: Cell) -> ResultsTableCell {
    match cell {
        Cell::Float(value) if value.is_finite() => ResultsTableCell::Number(value),
        Cell::Float(_) | Cell::Null => ResultsTableCell::Missing,
        Cell::Text(value) => ResultsTableCell::Text(value),
    }
}

fn measurement_row(
    result: &MetricResult,
    sweep: ResultsTableSweep,
    group_position: usize,
    group_span: usize,
) -> ResultsTableRow {
    let cells = ResultsTableColumn::ALL
        .iter()
        .copied()
        .map(|column| match column {
            ResultsTableColumn::Filename => ResultsTableCell::Text(result.filename.clone()),
            ResultsTableColumn::Sweep => ResultsTableCell::Sweep(sweep),
            _ => typed_cell(value_for_column(column.key(), result, sweep.report_name())),
        })
        .collect();
    ResultsTableRow {
        cells,
        sweep,
        kind: ResultsTableRowKind::Measurement,
        group_position,
        group_span,
    }
}

fn stat<'a>(stats: &'a [StatRow], scope: &str, metric: &str) -> Option<&'a StatRow> {
    stats
        .iter()
        .find(|row| row.scope == scope && row.metric == metric)
}

fn finite(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}

fn summary_cell(stats: &[StatRow], scope: &str, metric: &str) -> ResultsTableCell {
    let row = stat(stats, scope, metric);
    ResultsTableCell::Summary {
        mean: row.and_then(|row| finite(row.mean)),
        sample_std_dev: row.and_then(|row| finite(row.std)),
    }
}

fn log10_summary_cell(stats: &[StatRow], scope: &str, metric: &str) -> ResultsTableCell {
    let row = stat(stats, scope, metric);
    ResultsTableCell::Log10Summary {
        mean: row.and_then(|row| finite(row.mean)),
        sample_std_dev: row.and_then(|row| finite(row.std)),
    }
}

fn overall_row(
    stats: &[StatRow],
    sweep: ResultsTableSweep,
    group_position: usize,
) -> ResultsTableRow {
    let scope = sweep.report_name();
    let count = stat(stats, scope, "Vth").map_or(0, |row| row.count.max(0) as usize);
    let cells = ResultsTableColumn::ALL
        .iter()
        .copied()
        .map(|column| match column {
            ResultsTableColumn::Filename => ResultsTableCell::Overall,
            ResultsTableColumn::Sweep => ResultsTableCell::Sweep(sweep),
            ResultsTableColumn::WidthUm
            | ResultsTableColumn::LengthUm
            | ResultsTableColumn::AspectRatio
            | ResultsTableColumn::GeometrySource
            | ResultsTableColumn::Status => ResultsTableCell::Missing,
            ResultsTableColumn::ThresholdVoltage => summary_cell(stats, scope, "Vth"),
            ResultsTableColumn::SaturationMobility => summary_cell(stats, scope, "mu_sat"),
            ResultsTableColumn::SubthresholdSwing => summary_cell(stats, scope, "SS_mV_dec"),
            ResultsTableColumn::OnCurrent => summary_cell(stats, scope, "Ion"),
            ResultsTableColumn::OffCurrent => summary_cell(stats, scope, "Ioff"),
            ResultsTableColumn::OnOffRatio => log10_summary_cell(stats, scope, "log10_Ion_Ioff"),
            ResultsTableColumn::ThresholdHysteresis => {
                summary_cell(stats, "All", "DeltaVth_hysteresis")
            }
            ResultsTableColumn::Message => ResultsTableCell::SummaryCaption,
        })
        .collect();
    ResultsTableRow {
        cells,
        sweep,
        kind: ResultsTableRowKind::Overall { count },
        group_position,
        group_span: 2,
    }
}

pub(in crate::transfer) fn project_results_table(
    results: &[MetricResult],
) -> ResultsTableProjection {
    let mut rows = Vec::new();
    for result in results {
        let sweeps: &[ResultsTableSweep] = if result.has_backward_sweep {
            &[ResultsTableSweep::Forward, ResultsTableSweep::Backward]
        } else {
            &[ResultsTableSweep::Single]
        };
        for (group_position, &sweep) in sweeps.iter().enumerate() {
            rows.push(measurement_row(result, sweep, group_position, sweeps.len()));
        }
    }

    if !rows.is_empty() {
        let stats = results_to_stats(results);
        rows.push(overall_row(&stats, ResultsTableSweep::Forward, 0));
        rows.push(overall_row(&stats, ResultsTableSweep::Backward, 1));
    }

    ResultsTableProjection {
        columns: &ResultsTableColumn::ALL,
        rows,
    }
}
