//! Stable striped fill-table renderer.

use eframe::egui;

use super::{fill_card_width, header_rule, muted_header_label};

/// A plain striped table whose fixed column proportions scale up to span the
/// full card width - the label/value-table counterpart of the analytical grid
/// tables. The proportions never depend on loaded values, so switching data
/// cannot move the columns. `headers` empty => headerless body. Cells render via
/// `cell` (column index, markup) so callers keep their own colors; rows are
/// left-aligned like the geometry W/L table. The caller measures `card_w` BEFORE
/// opening any outer `ScrollArea` (the scroll content width is unbounded).
#[allow(clippy::too_many_arguments)]
pub fn striped_fill_table(
    ui: &mut egui::Ui,
    id: &str,
    headers: &[&str],
    base_ws: &[f32],
    rows: &[Vec<String>],
    card_w: f32,
    row_h: f32,
    mut cell: impl FnMut(&mut egui::Ui, usize, &str),
) {
    use egui_extras::{Column, TableBuilder};

    let ncols = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(headers.len());
    if ncols == 0 {
        return;
    }
    let gaps = (ncols.saturating_sub(1)) as f32 * ui.spacing().item_spacing.x;
    let mut widths: Vec<f32> = (0..ncols)
        .map(|i| base_ws.get(i).copied().unwrap_or(56.0))
        .collect();
    fill_card_width(&mut widths, (card_w - gaps).max(0.0));
    ui.set_min_width(widths.iter().sum::<f32>() + gaps);
    ui.push_id(id, |ui| {
        // No inter-row gap: stripes form contiguous bands, and the saved ~3.5px/row
        // is what lets all six sweep metrics fit the SELECTED card at rest.
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut builder = TableBuilder::new(ui)
            .striped(true)
            .vscroll(false)
            .auto_shrink([false, true])
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        for w in &widths {
            builder = builder.column(Column::exact(*w));
        }
        let render_body = |mut body: egui_extras::TableBody<'_>| {
            for row in rows {
                body.row(row_h, |mut tr| {
                    for i in 0..ncols {
                        tr.col(|ui| {
                            cell(ui, i, row.get(i).map(String::as_str).unwrap_or(""));
                        });
                    }
                });
            }
        };
        if headers.is_empty() {
            builder.body(render_body);
        } else {
            builder
                .header(row_h, |mut header| {
                    for h in headers {
                        header.col(|ui| {
                            muted_header_label(ui, h);
                            header_rule(ui);
                        });
                    }
                })
                .body(render_body);
        }
    });
}
