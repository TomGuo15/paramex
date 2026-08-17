//! Sheet-grid preparation and transfer-file rejection policy.

use crate::shared::grid_ingest::Grid;
use crate::transfer::file_name::output_name_hint;

use super::{find_column_by_label, ID_LABELS, VD_LABELS, VG_LABELS};

/// Output-curve markers live in the metadata preamble (first ~20 rows) by
/// B1500A convention; scanning further was 58% of parse time pre-fix
/// (`parser.py:29`, Phase 7 FOLLOW-UP #35).
pub(super) const OUTPUT_CURVE_SCAN_LIMIT: usize = 20;

/// Lower-cased, trimmed cells of grid row `row_idx` (`parser.py:294-295`,
/// `_normalized_row` = `str(cell).strip().lower()`). The grid cells are already
/// `str(value)` of the source cell, so this is just `trim().to_lowercase()`.
/// `row_idx` is assumed in-bounds (callers scan `0..min(limit, len)`).
pub(super) fn normalized_row(grid: &Grid, row_idx: usize) -> Vec<String> {
    grid[row_idx]
        .iter()
        .map(|cell| cell.trim().to_lowercase())
        .collect()
}

/// Detect a B1500A output-curve (`Id-Vd`) file so it is rejected from the
/// transfer path (`parser.py:298-325`). Content evidence decides first over the
/// scanned [`OUTPUT_CURVE_SCAN_LIMIT`] rows (load-bearing perf property): output
/// evidence rejects, and a transfer-shaped header row (Vg + Id, no Vd) clears a
/// filename that merely *looks* like an output convention — an Id-Vd grid always
/// carries a *recognized* Vd column (`VD_LABELS`), so the name hint (`id-vd` /
/// `-output` / digit+`o` stems) only decides when the scan finds neither.
pub(super) fn looks_like_output_curve(grid: &Grid, name: &str) -> bool {
    let name_hint = output_name_hint(name);
    let scan_rows = OUTPUT_CURVE_SCAN_LIMIT.min(grid.len());
    for row_idx in 0..scan_rows {
        let row = normalized_row(grid, row_idx);
        if row.is_empty() {
            continue;
        }
        let first_cell = &row[0];
        let vd_col = find_column_by_label(&row, &VD_LABELS);
        if (first_cell.contains("setup title") || first_cell.contains("setup name"))
            && row.iter().any(|v| v.replace('_', "-").contains("id-vd"))
        {
            return true;
        }
        if first_cell.contains("output.graph.xaxis.data") && vd_col.is_some() {
            return true;
        }
        let id_col = find_column_by_label(&row, &ID_LABELS);
        let vg_col = find_column_by_label(&row, &VG_LABELS);
        if vd_col.is_some() && id_col.is_some() && vg_col.is_none() {
            return true;
        }
        if name_hint && vg_col.is_some() && id_col.is_some() && vd_col.is_none() {
            return false;
        }
    }
    name_hint
}
