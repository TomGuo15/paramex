//! DATA input card: model choice plus loaders. Detailed model math belongs in
//! `paramex-core`, not in the working UI.

use eframe::egui;

use crate::io_tasks::IoQueue;
use crate::state::EditBuffers;
use crate::ui_kit::{self, Variant};
use crate::workspaces::modelfit::ingest::{
    start_add_cv_file, start_add_files, start_add_output_files, start_add_second_transfer, Msg,
};
use crate::workspaces::modelfit::models::FIT_MODELS;
use crate::workspaces::modelfit::state::ModelFitState;
use crate::workspaces::modelfit::ModelFitWorkspace;

pub const CARD_H: f32 = 157.0;

pub fn show(ui: &mut egui::Ui, workspace: &mut ModelFitWorkspace, edits: &mut EditBuffers) {
    let ModelFitWorkspace { state, io, .. } = workspace;
    let ctx = ui.ctx().clone();
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "DATA", None);
        show_body(ui, state, io, edits, &ctx);
    });
}

fn show_body(
    ui: &mut egui::Ui,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
    edits: &mut EditBuffers,
    ctx: &egui::Context,
) {
    let idle = io.is_idle();
    let current_model = state.selected_model();
    let row_width = ui.available_width();
    let row_height = ui.spacing().interact_size.y;
    let (row, _) = ui.allocate_exact_size(egui::vec2(row_width, row_height), egui::Sense::hover());
    let label_width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                "Model".to_owned(),
                egui::FontId::new(11.0, egui::FontFamily::Proportional),
                egui::Color32::TRANSPARENT,
            )
            .size()
            .x
    });
    let gap = ui.spacing().item_spacing.x;
    let label_rect = egui::Rect::from_min_size(row.min, egui::vec2(label_width, row_height))
        .translate(egui::vec2(0.0, 0.5));
    let picker_rect = egui::Rect::from_min_max(
        egui::pos2(row.left() + label_width + gap, row.top()),
        row.max,
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(label_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| ui_kit::field_label(ui, "Model"),
    );
    let picked = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(picker_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_width(picker_rect.width());
                ui.set_height(row_height);
                ui.add_enabled_ui(idle, |ui| model_dropdown(ui, current_model))
                    .inner
            },
        )
        .inner;
    if let Some(idx) = picked {
        // Drop pending parameter edits because the rendered parameter set changed.
        state.set_selected_model(idx);
        edits.forget_prefix("modelfit:p:");
    }
    ui.add_space(10.0);
    // Loaders stay VISIBLE but disabled while a load/export is in-flight (the
    // frozen Transfer/TLM idiom: `add_enabled_ui(idle, ..)`), rather than being
    // swapped for a "Loading…" label. A button that vanishes the instant it is
    // clicked reads as a bug — and the old label also appeared while the OS file
    // picker was still open, before anything was actually loading.
    let has_device = !state.is_empty();
    let mut load_files = false;
    let mut load_output = false;
    ui.columns(2, |cols| {
        load_files = cols[0]
            .add_enabled_ui(idle, |ui| {
                ui_kit::button_full(ui, "Load Transfer", Variant::Primary)
            })
            .inner
            .clicked();
        load_output = cols[1]
            .add_enabled_ui(idle, |ui| {
                ui_kit::button_full(ui, "Load Output", Variant::Primary)
            })
            .inner
            .clicked();
    });
    if load_files {
        start_add_files(ctx, io);
    }
    if load_output {
        start_add_output_files(ctx, io);
    }

    ui.add_space(4.0);
    let dibl_available = idle && has_device && state.selected_model_is_level62();
    let mut load_dibl = false;
    let mut load_cv = false;
    ui.columns(2, |cols| {
        load_dibl = cols[0]
            .add_enabled_ui(dibl_available, |ui| {
                ui_kit::button_full(ui, "Load DIBL", Variant::Secondary)
            })
            .inner
            .clicked();
        load_cv = cols[1]
            .add_enabled_ui(idle && has_device, |ui| {
                ui_kit::button_full(ui, "Load C-V", Variant::Secondary)
            })
            .inner
            .clicked();
    });
    if load_dibl {
        start_add_second_transfer(ctx, io, state.selected_device_id());
    }
    if load_cv {
        start_add_cv_file(ctx, io, state.selected_token());
    }
}

fn model_dropdown(ui: &mut egui::Ui, current: usize) -> Option<usize> {
    let mut picked = None;
    let current_label = FIT_MODELS.get(current).map_or("", |model| model.name);
    egui::ComboBox::from_id_salt("modelfit:model")
        .selected_text(current_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (idx, model) in FIT_MODELS.iter().enumerate() {
                if ui.selectable_label(idx == current, model.name).clicked() && idx != current {
                    picked = Some(idx);
                }
            }
        });
    picked
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        kittest::{NodeT, Queryable},
        Harness,
    };

    use super::*;
    use crate::io_tasks::spawn_io;
    use crate::workspaces::modelfit::models::LEVEL62_INDEX;

    struct BusyInputsApp {
        workspace: ModelFitWorkspace,
        edits: EditBuffers,
    }

    impl eframe::App for BusyInputsApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            show(ui, &mut self.workspace, &mut self.edits);
        }
    }

    #[test]
    fn model_and_load_actions_are_disabled_while_io_is_in_flight() {
        let mut state = ModelFitState::default();
        state.load_demo();
        assert!(state.set_selected_model(LEVEL62_INDEX));
        let mut workspace = ModelFitWorkspace::from_state(state);
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        spawn_io(
            &egui::Context::default(),
            &mut workspace.io,
            "blocked Model Fit test worker",
            move || -> Option<Msg> {
                let _ = release_rx.recv();
                None
            },
        );

        let state = BusyInputsApp {
            workspace,
            edits: EditBuffers::default(),
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(340.0, 640.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                state
            });
        harness.run();

        assert!(harness
            .get_all_by_role(egui::accesskit::Role::ComboBox)
            .next()
            .expect("model dropdown is present")
            .accesskit_node()
            .is_disabled());
        for label in ["Load Transfer", "Load Output", "Load DIBL", "Load C-V"] {
            assert!(
                harness.get_by_label(label).accesskit_node().is_disabled(),
                "{label} should be disabled while a Model Fit worker is running"
            );
        }

        release_tx.send(()).unwrap();
    }
}
