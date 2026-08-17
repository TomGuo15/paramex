//! Model Fit measurement-file parsing for output families, DIBL transfer pairs,
//! and accumulation-capacitance extraction.

use std::path::Path;

use super::types::OutputCurve;
use crate::shared::grid_headers::{find_column_by_label, next_row_has_numerics};
use crate::shared::grid_ingest::{
    coerce_numeric, normalized_extension, read_grids, HEADER_SCAN_LIMIT, MEASUREMENT_EXTENSIONS,
};
use crate::shared::numerics::{collapse_duplicate_x, median};
use crate::shared::output_measurement::{parse_raw_output_measurement, RawOutputMeasurement};

/// Parse output-curve bytes (csv/tsv/txt/xlsx) into Id-Vd sub-sweeps grouped by
/// `Vg`, each sorted ascending by `Vd`. `Id` is stored as a magnitude.
fn parse_output_bytes(content: &[u8], suffix: &str) -> Result<Vec<OutputCurve>, String> {
    let raw = parse_raw_output_measurement(content, suffix).map_err(|error| error.to_string())?;
    let curves = project_model_output(raw);
    if curves.is_empty() {
        return Err("no usable (Vg, Vd, Id) rows".to_string());
    }
    Ok(curves)
}

fn project_model_output(raw: RawOutputMeasurement) -> Vec<OutputCurve> {
    let mut groups: Vec<(f64, Vec<f64>, Vec<f64>)> = Vec::new();
    for sample in raw.samples {
        match groups
            .iter_mut()
            .find(|(vg, _, _)| (*vg - sample.vg).abs() < 1e-9)
        {
            Some((_, vds, ids)) => {
                vds.push(sample.vd);
                ids.push(sample.id.abs());
            }
            None => groups.push((sample.vg, vec![sample.vd], vec![sample.id.abs()])),
        }
    }

    groups
        .into_iter()
        .map(|(vg, vds, ids)| sorted_curve(vg, vds, ids))
        .collect()
}

/// A transfer sweep parsed together with its drain bias, read from the file's
/// constant drain-voltage column — the second-transfer input of the Level 62
/// DIBL refinement. `v_ds` keeps the measured (device-frame) sign;
/// the caller folds polarity.
#[derive(Debug, Clone, PartialEq)]
pub struct SecondTransfer {
    /// Gate-voltage samples (device frame, sweep order).
    pub vg: Vec<f64>,
    /// Drain-current magnitude at each `vg`.
    pub id_abs: Vec<f64>,
    /// The constant drain bias of the sweep (device frame, sign preserved).
    pub v_ds: f64,
}

/// Parse transfer-sweep bytes that carry their drain bias as a constant `Vd`
/// column (the B1500A `Id-Vg` export shape) into a [`SecondTransfer`]. Errors
/// when the file has no `(Vg, Vd, Id)` header, when the drain column varies
/// (that is an output sweep, not a transfer), or when the bias is zero.
fn parse_second_transfer_bytes(content: &[u8], suffix: &str) -> Result<SecondTransfer, String> {
    let raw = parse_raw_output_measurement(content, suffix).map_err(|error| error.to_string())?;
    let mut vg = Vec::new();
    let mut id_abs = Vec::new();
    let mut vd0: Option<f64> = None;
    for sample in raw.samples {
        let anchor = *vd0.get_or_insert(sample.vd);
        if (sample.vd - anchor).abs() > (1.0e-3 * anchor.abs()).max(1.0e-6) {
            return Err(format!(
                "the drain bias varies across the sweep (Vd {anchor} vs {}) \u{2014} \
                 this looks like an output (Id-Vd) file, not a transfer",
                sample.vd
            ));
        }
        vg.push(sample.vg);
        id_abs.push(sample.id.abs());
    }
    let v_ds = vd0.ok_or_else(|| "no usable (Vg, Vd, Id) rows".to_string())?;
    if v_ds.abs() < 1.0e-6 {
        return Err("the sweep's drain bias is zero \u{2014} no DIBL information".to_string());
    }
    if vg.len() < 10 {
        return Err("too few usable (Vg, Vd, Id) rows for a transfer".to_string());
    }
    Ok(SecondTransfer { vg, id_abs, v_ds })
}

/// Parse a second-transfer file by path (see [`parse_second_transfer_bytes`]).
pub fn parse_second_transfer_file(path: &Path) -> Result<SecondTransfer, String> {
    let (content, suffix) = read_measurement_file(path)?;
    parse_second_transfer_bytes(&content, &suffix)
}

/// Parse an output-curve file by path.
pub fn parse_output_file(path: &Path) -> Result<Vec<OutputCurve>, String> {
    let (content, suffix) = read_measurement_file(path)?;
    parse_output_bytes(&content, &suffix)
}

const BIAS_ALIASES: &[&str] = &["vbias", "vg", "vgs", "v_g", "vgate", "gate", "bias"];
const CAP_ALIASES: &[&str] = &["c", "cap", "cp", "cs", "cgg", "capacitance"];

/// Parse capacitance samples from C-V sweep bytes (csv/tsv/xls/xlsx). Tolerates
/// the same instrument preamble and misnamed-`.xls` quirks as the output parser.
fn parse_cv_bytes(content: &[u8], suffix: &str) -> Result<Vec<f64>, String> {
    let grids = read_grids(content, suffix).map_err(|e| e.0)?;
    let grid = grids
        .into_iter()
        .find(|g| !g.is_empty())
        .ok_or_else(|| "file has no rows".to_string())?;
    let (header_row, vb_col, c_col) = find_cv_header(&grid).ok_or_else(|| {
        "no header row with gate-bias (VBias) and capacitance (C) columns".to_string()
    })?;

    let mut c = Vec::new();
    for row in grid.iter().skip(header_row + 1) {
        if let (Some(_), Some(cv)) = (num(row, vb_col), num(row, c_col)) {
            c.push(cv);
        }
    }
    if c.is_empty() {
        return Err("no usable (VBias, C) rows".to_string());
    }
    Ok(c)
}

/// Parse a C-V sweep file by path.
fn parse_cv_file(path: &Path) -> Result<Vec<f64>, String> {
    let (content, suffix) = read_measurement_file(path)?;
    parse_cv_bytes(&content, &suffix)
}

/// Robust accumulation-plateau capacitance (Farads) from a C-V sweep: the median
/// of the top tertile of finite positive values. `None` for fewer than six usable
/// points.
fn accumulation_capacitance(capacitance: &[f64]) -> Option<f64> {
    let mut values: Vec<_> = capacitance
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect();
    if values.len() < 6 {
        return None;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let mut accumulation = values.split_off(values.len() * 2 / 3);
    median(&mut accumulation)
}

/// Read a C-V measurement and select its robust accumulation capacitance.
pub fn extract_accumulation_capacitance_file(path: &Path) -> Result<f64, String> {
    let capacitance = parse_cv_file(path)?;
    accumulation_capacitance(&capacitance)
        .ok_or_else(|| "no usable accumulation region in the C-V sweep".to_string())
}

/// First header row (within [`HEADER_SCAN_LIMIT`]) carrying both a gate-bias and a
/// capacitance column AND followed by a numeric data row. Returns `(row, bias_col,
/// cap_col)`.
fn find_cv_header(grid: &[Vec<String>]) -> Option<(usize, usize, usize)> {
    let scan = grid.len().min(HEADER_SCAN_LIMIT);
    (0..scan).find_map(|r| {
        let row = &grid[r];
        let vb = find_column_by_label(row, BIAS_ALIASES)?;
        let c = find_column_by_label(row, CAP_ALIASES)?;
        if vb == c {
            return None;
        }
        next_row_has_numerics(grid, r, &[vb, c]).then_some((r, vb, c))
    })
}

fn num(row: &[String], col: usize) -> Option<f64> {
    let v = coerce_numeric(row.get(col)?);
    v.is_finite().then_some(v)
}

fn read_measurement_file(path: &Path) -> Result<(Vec<u8>, String), String> {
    let suffix = normalized_extension(path);
    if !MEASUREMENT_EXTENSIONS.contains(&suffix.as_str()) {
        return Err(format!("Unsupported file extension: {suffix}"));
    }
    std::fs::read(path)
        .map(|content| (content, suffix))
        .map_err(|error| error.to_string())
}

fn sorted_curve(vg: f64, vds: Vec<f64>, ids: Vec<f64>) -> OutputCurve {
    // ponytail: average forward/reverse branches at equal Vd; split them only if
    // output-hysteresis fitting becomes a product requirement.
    let (vds, id) = collapse_duplicate_x(&vds, &ids);
    OutputCurve { vg, vds, id }
}

#[cfg(test)]
#[path = "tests/parse.rs"]
mod integration_tests;
