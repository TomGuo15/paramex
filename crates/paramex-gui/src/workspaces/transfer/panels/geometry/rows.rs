//! Per-file geometry row rendering and commit policy.

use eframe::egui;
use egui_extras::Column;
use egui_notify::Toasts;
use paramex_core::transfer::Session;

use crate::format_ui::{WL_NUMERIC_MESSAGE, WL_POSITIVE_MESSAGE};
use crate::state::EditBuffers;
use crate::table_kit;
use crate::ui_kit;

const GEOMETRY_COL_GALLEYS: [f32; 3] = [32.0, 38.0, 38.0];
const GEOMETRY_COL_FLOORS: [f32; 3] = [96.0, 64.0, 64.0];

/// Commit one per-file W/L edit through `Session`: preserve the unedited
/// dimension (`None`), set `source="manual"`, and recompute just this file.
/// Returns the validation error (`"W and L must be positive."`) without
/// mutating on failure.
pub fn commit_row_geometry(
    session: &mut Session,
    file_id: &str,
    width: Option<f64>,
    length: Option<f64>,
) -> Result<(), String> {
    session
        .set_manual_geometry(file_id, width, length)
        .map(|_| ())
}

pub(super) fn render_geometry_rows_section(
    ui: &mut egui::Ui,
    session: &mut Session,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    let table_h = ui.available_height().max(0.0);
    render_file_table(ui, session, edits, toasts, table_h);
}

/// The per-file W/L grid. Uses the deferred-action pattern: snapshot display
/// rows first, collect edits during render, then apply commits through
/// `Session`.
fn render_file_table(
    ui: &mut egui::Ui,
    session: &mut Session,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    table_h: f32,
) {
    let rows = session.file_geometry_rows();

    let mut commits: Vec<(String, Option<f64>, Option<f64>)> = Vec::new();
    let mut numeric_errors = 0u32;

    let widths = table_kit::fit_fill_widths(
        &GEOMETRY_COL_GALLEYS,
        &GEOMETRY_COL_FLOORS,
        ui.available_width(),
        ui.spacing().item_spacing.x,
    );
    table_kit::quiet_table_builder(ui, table_h - table_kit::ROW_H)
        .column(Column::exact(widths[0]).at_least(widths[0]).clip(true))
        .column(Column::exact(widths[1]).at_least(widths[1]))
        .column(Column::exact(widths[2]).at_least(widths[2]))
        .header(table_kit::ROW_H, |mut header| {
            for label in ["File", "W (\u{00B5}m)", "L (\u{00B5}m)"] {
                header.col(|ui| {
                    table_kit::aligned_cell(ui, false, |ui| {
                        table_kit::muted_header_label(ui, label)
                    });
                    table_kit::header_rule(ui);
                });
            }
        })
        .body(|body| {
            body.rows(table_kit::ROW_H, rows.len(), |mut row| {
                let geometry = &rows[row.index()];
                row.col(|ui| {
                    table_kit::aligned_cell(ui, false, |ui| {
                        if geometry.source == "manual" {
                            ui_kit::semantic_badge(ui, "manual", ui_kit::BadgeTone::Warning);
                        }
                        let resp = table_kit::body_label(ui, &geometry.name);
                        table_kit::hover_if_clipped(ui, resp, &geometry.name);
                    });
                });
                row.col(|ui| {
                    table_kit::aligned_cell(ui, false, |ui| {
                        edit_dim(
                            ui,
                            edits,
                            &format!("geom:{}:w", geometry.file_id),
                            geometry.width_um,
                            |v| commits.push((geometry.file_id.clone(), Some(v), None)),
                            &mut numeric_errors,
                        );
                    });
                });
                row.col(|ui| {
                    table_kit::aligned_cell(ui, false, |ui| {
                        edit_dim(
                            ui,
                            edits,
                            &format!("geom:{}:l", geometry.file_id),
                            geometry.length_um,
                            |v| commits.push((geometry.file_id.clone(), None, Some(v))),
                            &mut numeric_errors,
                        );
                    });
                });
            });
        });

    for (id, width, length) in commits {
        if commit_row_geometry(session, &id, width, length).is_err() {
            toasts.warning(WL_POSITIVE_MESSAGE);
        }
    }
    for _ in 0..numeric_errors {
        toasts.warning(WL_NUMERIC_MESSAGE); // geometry.py:112 parse failure
    }
}

/// One focus-tracked numeric input. On `lost_focus`, parse + invoke `on_commit`
/// (Ok) or bump `numeric_errors` (parse failure). While unfocused, drop the buffer
/// so it re-syncs to the committed value next frame.
fn edit_dim(
    ui: &mut egui::Ui,
    edits: &mut EditBuffers,
    key: &str,
    current: f64,
    mut on_commit: impl FnMut(f64),
    numeric_errors: &mut u32,
) {
    let current_str = format!("{current}");
    let width = ui.available_width();
    if let Some(text) = ui_kit::singleline_edit_commit(ui, edits, key, &current_str, width) {
        match text.trim().parse::<f64>() {
            Ok(value) => on_commit(value),
            Err(_) => *numeric_errors += 1,
        }
    }
}
