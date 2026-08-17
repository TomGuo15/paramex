//! Raw long-form output-measurement ingestion shared by product seams.
//!
//! This module interprets file structure only. Product modules decide how to
//! group, order, sign-fold, and deduplicate the returned source-order samples.

use std::fmt;

use super::grid_headers::find_column_by_label;
use super::grid_ingest::{coerce_numeric, read_grids, split_single_column, HEADER_SCAN_LIMIT};

const VG_ALIASES: &[&str] = &[
    "vg",
    "vgs",
    "v_g",
    "gate",
    "gatev",
    "gate voltage",
    "gate_voltage",
];
const VD_ALIASES: &[&str] = &[
    "vd",
    "vds",
    "v_d",
    "drainv",
    "drain voltage",
    "drain_voltage",
];
const SIGNED_ID_ALIASES: &[&str] = &[
    "id",
    "ids",
    "i_d",
    "drain",
    "draini",
    "drain current",
    "drain_current",
];
const MAGNITUDE_ID_ALIASES: &[&str] = &["abs_id", "absid"];

/// Meaning of the drain-current column selected from the source header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrainCurrentColumn {
    /// A signed terminal-current column such as `Id`.
    Signed,
    /// A magnitude column such as `abs_Id`.
    Magnitude,
}

/// One finite long-form output sample, retained in source-row order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RawOutputSample {
    pub(crate) vg: f64,
    pub(crate) vd: f64,
    /// The source value without sign folding or magnitude conversion.
    pub(crate) id: f64,
}

/// One parsed long-form `(Vg, Vd, Id)` measurement.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawOutputMeasurement {
    pub(crate) samples: Vec<RawOutputSample>,
    pub(crate) current_column: DrainCurrentColumn,
}

/// Structural failures from raw output-measurement ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawOutputParseError {
    GridRead(String),
    NoRows,
    MissingColumns,
    NoSamples,
}

impl fmt::Display for RawOutputParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GridRead(message) => f.write_str(message),
            Self::NoRows => f.write_str("file has no rows"),
            Self::MissingColumns => f.write_str(
                "no header row with gate-voltage (Vg), drain-voltage (Vd), and \
                 drain-current (Id) columns",
            ),
            Self::NoSamples => f.write_str("no usable (Vg, Vd, Id) rows"),
        }
    }
}

impl std::error::Error for RawOutputParseError {}

/// Parse every usable long-form output measurement from ordered file grids.
///
/// At most one measurement is returned per grid, in grid order. The
/// signed-current vocabulary is searched before the magnitude vocabulary,
/// independent of physical column order. Invalid rows are skipped; retained
/// finite samples remain in their original row order.
pub(crate) fn parse_raw_output_measurements(
    content: &[u8],
    suffix: &str,
) -> Result<Vec<RawOutputMeasurement>, RawOutputParseError> {
    let suffix = suffix.to_ascii_lowercase();
    let grids =
        read_grids(content, &suffix).map_err(|error| RawOutputParseError::GridRead(error.0))?;
    let mut saw_rows = false;
    let mut saw_header = false;
    let mut measurements = Vec::new();

    for grid in grids {
        let grid = split_single_column(&grid);
        if grid.is_empty() {
            continue;
        }
        saw_rows = true;

        for header_row in 0..grid.len().min(HEADER_SCAN_LIMIT) {
            let Some((vg_col, vd_col, id_col, current_column)) = output_columns(&grid[header_row])
            else {
                continue;
            };
            saw_header = true;

            let samples: Vec<RawOutputSample> = grid
                .iter()
                .skip(header_row + 1)
                .filter_map(|row| {
                    let vg = finite_cell(row, vg_col)?;
                    let vd = finite_cell(row, vd_col)?;
                    let id = finite_cell(row, id_col)?;
                    Some(RawOutputSample { vg, vd, id })
                })
                .collect();
            if !samples.is_empty() {
                measurements.push(RawOutputMeasurement {
                    samples,
                    current_column,
                });
                break;
            }
        }
    }

    if !measurements.is_empty() {
        Ok(measurements)
    } else if !saw_rows {
        Err(RawOutputParseError::NoRows)
    } else if saw_header {
        Err(RawOutputParseError::NoSamples)
    } else {
        Err(RawOutputParseError::MissingColumns)
    }
}

/// Parse the first usable long-form output measurement in grid order.
pub(crate) fn parse_raw_output_measurement(
    content: &[u8],
    suffix: &str,
) -> Result<RawOutputMeasurement, RawOutputParseError> {
    Ok(parse_raw_output_measurements(content, suffix)?
        .into_iter()
        .next()
        .expect("successful raw output parse has at least one measurement"))
}

fn output_columns(row: &[String]) -> Option<(usize, usize, usize, DrainCurrentColumn)> {
    let vg = find_column_by_label(row, VG_ALIASES)?;
    let vd = find_column_by_label(row, VD_ALIASES)?;
    let (id, current_column) = if let Some(id) = find_column_by_label(row, SIGNED_ID_ALIASES) {
        (id, DrainCurrentColumn::Signed)
    } else {
        (
            find_column_by_label(row, MAGNITUDE_ID_ALIASES)?,
            DrainCurrentColumn::Magnitude,
        )
    };
    (vg != vd && vg != id && vd != id).then_some((vg, vd, id, current_column))
}

fn finite_cell(row: &[String], column: usize) -> Option<f64> {
    let value = coerce_numeric(row.get(column)?);
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_signed_current_and_preserves_source_order() {
        let csv = b"Instrument,output sweep\n\
                    Vg (V),Vd (V),abs_Id (A),Id (A)\n\
                    2,2,20,-2\n\
                    ignored,row\n\
                    2,0,10,-1\n\
                    2,0,30,-3\n";

        let raw = parse_raw_output_measurement(csv, ".CSV").expect("raw output parses");

        assert_eq!(raw.current_column, DrainCurrentColumn::Signed);
        assert_eq!(
            raw.samples,
            vec![
                RawOutputSample {
                    vg: 2.0,
                    vd: 2.0,
                    id: -2.0,
                },
                RawOutputSample {
                    vg: 2.0,
                    vd: 0.0,
                    id: -1.0,
                },
                RawOutputSample {
                    vg: 2.0,
                    vd: 0.0,
                    id: -3.0,
                },
            ]
        );
    }

    #[test]
    fn records_magnitude_provenance_for_single_column_text() {
        let txt = b"Vg Vd abs_Id\n2 2 2e-6\n2 0 0\n";

        let raw = parse_raw_output_measurement(txt, ".txt").expect("raw text output parses");

        assert_eq!(raw.current_column, DrainCurrentColumn::Magnitude);
        assert_eq!(
            raw.samples,
            vec![
                RawOutputSample {
                    vg: 2.0,
                    vd: 2.0,
                    id: 2.0e-6,
                },
                RawOutputSample {
                    vg: 2.0,
                    vd: 0.0,
                    id: 0.0,
                },
            ]
        );
    }
}
