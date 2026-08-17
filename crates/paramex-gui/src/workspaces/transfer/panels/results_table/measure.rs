//! Results-table column measurement.

use eframe::egui;

use super::columns;
use super::columns::GuiColumnSpec;
use crate::table_kit::{fill_card_width, galley_width, pad_and_clamp};

pub(super) struct FittedWidths {
    pub column_widths: Vec<f32>,
    pub painted_w: f32,
}

/// Measure each column's stable render width from its header/schema only.
/// Row contents clip on overflow instead of moving the column grid after load.
pub(super) fn measure_col_widths(ui: &egui::Ui, indexed_specs: &[GuiColumnSpec]) -> Vec<f32> {
    // Headers measure at the muted 11px font they render in (same rule as
    // table_kit::measure_grid_col_galleys).
    let header_font = crate::table_kit::muted_header_font();
    indexed_specs
        .iter()
        .map(|spec| {
            let w = galley_width(ui, columns::gui_header_label_html(spec), &header_font);
            pad_and_clamp(w, columns::col_min_width(spec))
        })
        .collect()
}

pub(super) fn fit_table_widths(
    ui: &egui::Ui,
    measured: &[f32],
    indexed_specs: &[GuiColumnSpec],
    card_w: f32,
) -> FittedWidths {
    // Columns are sized from stable schema/header widths. The live card width
    // changes per frame, so fit a local copy before rendering.
    let mut column_widths = measured.to_vec();
    // Gap-aware fill (user 2026-06-12, revising the 2026-06-10 overshoot rule):
    // the PAINTED table is columns plus inter-column gaps, so fill against the
    // card width minus those gaps. Only genuinely wider content should keep the
    // mid-cell cut and horizontal scroll.
    let gaps = indexed_specs.len().saturating_sub(1) as f32 * ui.spacing().item_spacing.x;
    let avail = (card_w - gaps).max(0.0);
    let min_widths: Vec<f32> = indexed_specs.iter().map(columns::col_min_width).collect();
    shrink_widths_to_fit(&mut column_widths, &min_widths, avail);
    fill_card_width(&mut column_widths, avail);
    // Declare the full table width so the narrow centre card scrolls
    // horizontally. Group separators span the painted width (columns + gaps).
    let table_w = column_widths
        .iter()
        .sum::<f32>()
        .max(columns::table_min_width());
    let painted_w = table_w + gaps;
    FittedWidths {
        column_widths,
        painted_w,
    }
}

fn shrink_widths_to_fit(widths: &mut [f32], min_widths: &[f32], avail: f32) {
    let current = widths.iter().sum::<f32>();
    if current <= avail {
        return;
    }
    let min_sum = min_widths.iter().sum::<f32>();
    if min_sum > avail {
        return;
    }
    let shrink_capacity = widths
        .iter()
        .zip(min_widths.iter())
        .map(|(width, floor)| (width - floor).max(0.0))
        .sum::<f32>();
    if shrink_capacity <= 0.0 {
        return;
    }
    let excess = current - avail;
    for (width, floor) in widths.iter_mut().zip(min_widths.iter()) {
        let capacity = (*width - *floor).max(0.0);
        *width = (*width - excess * capacity / shrink_capacity).max(*floor);
    }
}
