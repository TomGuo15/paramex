//! Model Fit workspace page composition.

use eframe::egui;
use egui_notify::Toasts;

use super::{panels, state::ModelFitState};
use crate::layout::{self, ShellRects};
use crate::state::EditBuffers;
use crate::ui_kit;
use crate::workspaces::modelfit::ModelFitWorkspace;

/// Minimum height of one center graph tile before the center column scrolls.
const PLOT_TILE_MIN_H: f32 = 216.0;

pub fn show(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    shell: &ShellRects,
    workspace: &mut ModelFitWorkspace,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    layout::show_in_rect(ui, "modelfit_left_rect", shell.left, |ui| {
        show_left_column(ui, workspace, edits, toasts);
    });
    layout::show_in_rect(ui, "modelfit_center_rect", shell.center, |ui| {
        show_center_column(ui, workspace);
    });
    layout::show_in_rect(ui, "modelfit_right_rect", shell.right, |ui| {
        show_right_column(ui, workspace, edits, toasts);
    });
}

fn show_left_column(
    ui: &mut egui::Ui,
    workspace: &mut ModelFitWorkspace,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    layout::prepare_column(ui);
    // The inputs panel owns its capped body scroll so the card chrome stays fixed.
    ui.allocate_ui(
        egui::vec2(ui.available_width(), panels::inputs::CARD_H),
        |ui| {
            panels::inputs::show(ui, workspace, edits);
        },
    );
    ui.add_space(layout::CARD_GAP);
    let devices_h = ui.available_height().max(layout::CONTENT_CARD_MIN_HEIGHT);
    let actions_enabled = workspace.io.is_idle();
    ui.allocate_ui(egui::vec2(ui.available_width(), devices_h), |ui| {
        panels::devices::show(ui, workspace, edits, toasts, actions_enabled);
    });
}

fn show_center_column(ui: &mut egui::Ui, workspace: &ModelFitWorkspace) {
    // The center is GRAPHS ONLY now: a 2-wide grid of six plot tiles —
    // (1) Transfer FIT, (2) OUTPUT, (3) g_m-V_G, (4) g_ds-V_D,
    // (5) g_m/I_D sizing curve, (6) GAIN (A_v=g_m/g_ds) — drawn together (no
    // toggle/tabs). The rows fill the center column at normal/tall sizes and scroll only
    // on genuinely short windows.
    layout::prepare_column(ui);
    let col_h = ui.available_height();
    let state = &workspace.state;
    let row_h = plot_tile_height(col_h);
    if plot_grid_needs_scroll(col_h) {
        ui_kit::scroll_body(ui, "modelfit_center_col", col_h, |ui| {
            show_plot_rows(ui, state, row_h);
        });
    } else {
        show_plot_rows(ui, state, row_h);
    }
}

fn plot_tile_height(available_height: f32) -> f32 {
    ((available_height - 2.0 * layout::CARD_GAP) / 3.0).max(PLOT_TILE_MIN_H)
}

fn plot_grid_height(row_h: f32) -> f32 {
    row_h * 3.0 + 2.0 * layout::CARD_GAP
}

fn plot_grid_needs_scroll(available_height: f32) -> bool {
    available_height < plot_grid_height(PLOT_TILE_MIN_H)
}

fn show_plot_rows(ui: &mut egui::Ui, state: &ModelFitState, row_h: f32) {
    plot_row(ui, row_h, |left, right| {
        panels::plot::show(left, state);
        panels::output_plot::show(right, state);
    });
    ui.add_space(layout::CARD_GAP);
    plot_row(ui, row_h, |left, right| {
        panels::gm_plot::show(left, state);
        panels::gds_plot::show(right, state);
    });
    ui.add_space(layout::CARD_GAP);
    plot_row(ui, row_h, |left, right| {
        panels::gmid_plot::show(left, state);
        panels::gain_plot::show(right, state);
    });
}

/// One row of the center grid: two equal-width plot tiles side by side, each a
/// fixed-height slot so its `card_slot` host fills a readable area.
fn plot_row(ui: &mut egui::Ui, height: f32, add: impl FnOnce(&mut egui::Ui, &mut egui::Ui)) {
    let row_w = ui.available_width();
    let (row_rect, _) = ui.allocate_exact_size(egui::vec2(row_w, height), egui::Sense::hover());
    let tile_w = ((row_w - layout::CARD_GAP) * 0.5).max(0.0);
    let left_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(tile_w, height));
    let right_rect = egui::Rect::from_min_size(
        egui::pos2(left_rect.right() + layout::CARD_GAP, row_rect.top()),
        egui::vec2(tile_w, height),
    );
    let mut left_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left_rect)
            .layout(*ui.layout()),
    );
    let mut right_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(right_rect)
            .layout(*ui.layout()),
    );
    left_ui.set_min_size(left_rect.size());
    right_ui.set_min_size(right_rect.size());
    add(&mut left_ui, &mut right_ui);
}

fn show_right_column(
    ui: &mut egui::Ui,
    workspace: &mut ModelFitWorkspace,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    layout::prepare_column(ui);
    panels::summary::show_parameters(ui, workspace, edits, toasts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_plot_rows_fill_available_height_before_scrolling() {
        let reference = 720.0;
        let h = plot_tile_height(reference);
        assert!((plot_grid_height(h) - reference).abs() < 0.01);
        assert!(plot_tile_height(900.0) > h);
        assert_eq!(plot_tile_height(560.0), PLOT_TILE_MIN_H);
        assert!(plot_grid_height(PLOT_TILE_MIN_H) > 560.0);
        assert!(plot_grid_needs_scroll(679.5));
        assert!(!plot_grid_needs_scroll(680.0));
    }
}
