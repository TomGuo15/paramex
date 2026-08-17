//! Right-column geometry panel (`geometry_panel.py`): a per-file W/L table and a
//! global W·L apply. Committed state is `Session`; per-file edit text lives in
//! `EditBuffers` (commit on focus-loss).

use eframe::egui;
use egui_notify::Toasts;
use paramex_core::transfer::Session;

mod rows;

use crate::format_ui::{global_wl_message, WL_NUMERIC_MESSAGE, WL_POSITIVE_MESSAGE};
use crate::state::EditBuffers;
use crate::ui_kit::{self, Variant};
use crate::workspaces::transfer::state::GeometryUi;

pub use rows::commit_row_geometry;

pub fn show_setup(
    ui: &mut egui::Ui,
    session: &mut Session,
    geo: &mut GeometryUi,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "GEOMETRY", None);
        render_global_wl_controls(ui, session, geo, edits, toasts);
        ui.add_space(8.0);
        rows::render_geometry_rows_section(ui, session, edits, toasts);
    });
}

fn render_global_wl_controls(
    ui: &mut egui::Ui,
    session: &mut Session,
    geo: &mut GeometryUi,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    let (global_w, global_l) = geo.global_wl_mut();
    let row_w = ui.available_width();
    ui_kit::inline_paired_settings_row_sized(
        ui,
        row_w,
        "W (\u{00B5}m)",
        global_w,
        "L (\u{00B5}m)",
        global_l,
    );
    ui.add_space(8.0);
    let apply_clicked = ui
        .add_enabled_ui(session.has_files(), |ui| {
            ui_kit::button_full(ui, "Apply W/L to All Files", Variant::Secondary)
        })
        .inner
        .clicked();
    if apply_clicked {
        // Distinguish non-numeric text from non-positive numbers, like the
        // per-row editor below. (The old unwrap_or(0.0) reported "abc" as
        // "must be positive" — a port of Python's number widgets, where
        // non-numeric text was impossible; these are free-text fields.)
        match geo.parse_global_wl() {
            Some((w, l)) => match session.set_global_wl(w, l) {
                Ok(count) => {
                    // The apply changed every file's committed W/L. Drop the per-file geom
                    // buffers so a field that was focused at click time (its lost_focus commit
                    // fires THIS frame) can't override the apply with its now-stale value — the
                    // shared changed-text guard can't catch that case, because the stale buffer
                    // DOES differ from the just-applied value.
                    edits.forget_prefix("geom:");
                    toasts.success(global_wl_message(count));
                }
                Err(_message) => {
                    toasts.warning(WL_POSITIVE_MESSAGE);
                }
            },
            None => {
                toasts.warning(WL_NUMERIC_MESSAGE); // geometry.py:112
            }
        }
    }
}
