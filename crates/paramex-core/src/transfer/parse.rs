//! Transfer-curve file parser — the GUI's file-upload and disk-load paths.
//! Mirrors `paramex.core.parsing` (pure label helpers) + the Transfer
//! `extraction.parser` (sheet/grid scan, rejection filters, curve build).
//!
//! # The `Grid` boundary
//! All scan/detect/build logic is pure over a [`Grid`] (one sheet:
//! `Vec<Vec<String>>`). The `grid_ingest` module turns file bytes into ordered
//! grids (one per sheet; one for delimited text). This split is what makes the
//! science golden-testable against the Python oracle: the same string grid
//! Python derives from a pandas DataFrame is fed to the Rust logic.

use std::path::{Path, PathBuf};

use crate::shared::grid_headers::find_column_by_label;
use crate::shared::grid_ingest::{normalized_extension, split_single_column, Grid};
use curve::build_curve;
pub(in crate::transfer) use curve::validate_curve_integrity;
pub use io::{parse_transfer_bytes, parse_transfer_file, ParseError};
use labels::{parse_labeled_columns, ID_LABELS, VD_LABELS, VG_LABELS};
use numeric::parse_numeric_fallback;
#[cfg(test)]
use numeric::{looks_like_scope_trace, looks_like_spectrum_trace};
#[cfg(test)]
use sheet::OUTPUT_CURVE_SCAN_LIMIT;
use sheet::{looks_like_output_curve, normalized_row};

mod curve;
mod io;
mod labels;
mod numeric;
mod sheet;

#[cfg(test)]
mod tests;

/// File extensions the parser accepts (`parser.py:19`). Compared against the
/// lower-cased extension *including* the leading dot.
pub const SUPPORTED_EXTENSIONS: [&str; 5] = crate::shared::grid_ingest::MEASUREMENT_EXTENSIONS;

/// Whether a path has a supported Transfer measurement-file extension.
pub fn is_supported_measurement_path(path: &Path) -> bool {
    let suffix = normalized_extension(path);
    SUPPORTED_EXTENSIONS.contains(&suffix.as_str())
}

pub(super) fn read_measurement_file(path: &Path) -> Result<(String, String, Vec<u8>), ParseError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let suffix = normalized_extension(path);
    if !is_supported_measurement_path(path) {
        return Err(ParseError(format!("Unsupported file extension: {suffix}")));
    }
    let content = std::fs::read(path)
        .map_err(|e| ParseError(format!("Could not read {}: {e}", path.display())))?;
    Ok((name, suffix, content))
}

/// Labeled headers can live deep in B1500A files (a real fixture puts one at
/// row 121), so the header scan is generous (`parser.py:23`).
const HEADER_SCAN_LIMIT: usize = crate::shared::grid_ingest::HEADER_SCAN_LIMIT;

/// Minimum finite positive-current rows for a usable transfer curve
/// (`parser.py:30`).
pub const MIN_TRANSFER_POINTS: usize = 12;

use crate::transfer::types::ParsedCurve;

/// Parse one sheet's grid into a [`ParsedCurve`] (`parser.py:147-157`,
/// `_parse_dataframe`). Empty → `None`; split a single delimited column; reject
/// output curves; try labeled columns, else the numeric fallback.
fn parse_grid(grid: &Grid, name: &str, source_path: Option<PathBuf>) -> Option<ParsedCurve> {
    // `df.empty` is true when there are no rows OR no columns.
    if grid.is_empty() || grid.iter().all(|row| row.is_empty()) {
        return None;
    }
    let g = split_single_column(grid);
    if looks_like_output_curve(&g, name) {
        return None;
    }
    if let Some(curve) = parse_labeled_columns(&g, name, source_path.clone()) {
        return Some(curve);
    }
    parse_numeric_fallback(&g, name, source_path)
}
