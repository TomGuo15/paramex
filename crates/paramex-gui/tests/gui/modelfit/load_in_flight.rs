//! Model Fit loaders gate on their workspace-owned queue, so a stale in-flight
//! worker cannot re-add devices after Clear All. Snapshots render at rest, so
//! these guards cover the loader layout and availability policy directly.

use crate::common;
use eframe::egui;
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use paramex_gui::state::EditBuffers;
use paramex_gui::ui_kit::CARD_INNER_MARGIN;
use paramex_gui::workspaces::modelfit::models::LEVEL62_INDEX;
use paramex_gui::workspaces::modelfit::panels::inputs::CARD_H;
use paramex_gui::workspaces::modelfit::state::ModelFitState;
use paramex_gui::workspaces::modelfit::ModelFitWorkspace;

struct InputsApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    size: egui::Vec2,
}

impl eframe::App for InputsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            paramex_gui::workspaces::modelfit::panels::inputs::show(
                ui,
                &mut self.workspace,
                &mut self.edits,
            );
        });
    }
}

#[test]
fn model_label_and_picker_share_a_raster_baseline_at_fractional_dpi() {
    for pixels_per_point in common::RASTER_TEST_SCALES {
        let app = InputsApp {
            workspace: ModelFitWorkspace::default(),
            edits: EditBuffers::default(),
            size: egui::Vec2::new(300.0, 600.0),
        };
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(340.0, 640.0))
            .with_pixels_per_point(pixels_per_point)
            .build_eframe(|cc| {
                paramex_gui::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        let label = harness.get_by_label("Model").rect();
        let picker = harness
            .get_all_by_role(egui::accesskit::Role::ComboBox)
            .next()
            .expect("Model Fit DATA should render one model picker")
            .rect();
        common::assert_raster_centers_aligned(
            &format!("Model label/picker baseline at {pixels_per_point} ppp"),
            label.center().y,
            picker.center().y,
            pixels_per_point,
        );
    }
}
#[test]
fn model_fit_optional_loaders_share_an_inline_row() {
    let state = crate::common::modelfit::demo_state();
    let app = InputsApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        size: egui::Vec2::new(300.0, 600.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(340.0, 640.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    let files = harness.get_by_label("Load Transfer").rect();
    let output = harness.get_by_label("Load Output").rect();
    let dibl = harness.get_by_label("Load DIBL").rect();
    let cv = harness.get_by_label("Load C-V").rect();
    let model_label = harness.get_by_label("Model").rect();
    let model_picker = harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .next()
        .expect("Model Fit DATA should render one model picker")
        .rect();
    let pixels_per_point = harness.ctx.pixels_per_point();

    common::assert_raster_centers_aligned(
        "Model label and picker baseline",
        model_label.center().y,
        model_picker.center().y,
        pixels_per_point,
    );

    common::assert_same_raster_edge(
        "Model Fit primary-action top",
        files.top(),
        output.top(),
        pixels_per_point,
    );
    common::assert_same_raster_edge(
        "Model Fit primary-action bottom",
        files.bottom(),
        output.bottom(),
        pixels_per_point,
    );
    assert!(
        files.right() <= output.left() - 1.0,
        "Model Fit primary actions should split the row left-to-right: transfer={files:?}, output={output:?}"
    );
    common::assert_same_raster_edge(
        "Model Fit optional-action top",
        dibl.top(),
        cv.top(),
        pixels_per_point,
    );
    common::assert_same_raster_edge(
        "Model Fit optional-action bottom",
        dibl.bottom(),
        cv.bottom(),
        pixels_per_point,
    );
    assert!(
        dibl.right() <= cv.left() - 1.0,
        "Model Fit optional actions should split the row left-to-right: dibl={dibl:?}, cv={cv:?}"
    );
}

#[test]
fn dibl_loader_is_available_only_for_loaded_level62() {
    for (case, loaded, level62, expected_disabled) in [
        ("loaded AOSTFT", true, false, true),
        ("empty Level 62", false, true, true),
        ("loaded Level 62", true, true, false),
    ] {
        let mut state = if loaded {
            crate::common::modelfit::demo_state()
        } else {
            ModelFitState::default()
        };
        if level62 {
            assert!(state.set_selected_model(LEVEL62_INDEX));
        }
        let app = InputsApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            size: egui::Vec2::new(300.0, 600.0),
        };
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(340.0, 640.0))
            .build_eframe(|cc| {
                paramex_gui::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        assert_eq!(
            harness
                .get_by_label("Load DIBL")
                .accesskit_node()
                .is_disabled(),
            expected_disabled,
            "Load DIBL availability is wrong for {case}"
        );
    }
}

#[test]
fn model_fit_load_actions_fit_the_fixed_card_height() {
    let app = InputsApp {
        workspace: ModelFitWorkspace::default(),
        edits: EditBuffers::default(),
        size: egui::Vec2::new(300.0, CARD_H),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(340.0, CARD_H + 40.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    let title = harness.get_by_label("DATA").rect();
    let cv = harness.get_by_label("Load C-V").rect();
    assert!(
        !harness
            .get_by_label("Load Output")
            .accesskit_node()
            .is_disabled(),
        "output-first loading stays available and creates a pending row"
    );
    let inner_bottom = title.top() - CARD_INNER_MARGIN as f32 + CARD_H - CARD_INNER_MARGIN as f32;
    assert!(
        common::raster_pixel(cv.bottom()) <= common::raster_pixel(inner_bottom),
        "Model Fit DATA card clips its final loader row: cv={cv:?}, inner_bottom={inner_bottom:.1}"
    );
    assert!(
        inner_bottom - cv.bottom() <= 6.0,
        "Model Fit DATA card leaves excess space below its final loader row: cv={cv:?}, inner_bottom={inner_bottom:.1}"
    );
}
