//! Pointer-driven guard for the editable AOSTFT PARAMETERS card: once a device is in
//! manual mode, the "Reset to Auto" button must RENDER and, on a real click, re-run
//! the extraction and clear manual mode (snapshots cannot prove the click commits).
//! The field commit gate itself is covered by `edit_buffers` / `geometry_focus`; the commit
//! handlers by the `summary` unit tests; this locks the reset wiring through a real button.

use crate::common;
use eframe::egui;
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use egui_notify::Toasts;
use paramex_core::modelfit::{FitModel, ModelParams};
use paramex_core::transfer::Session;
use paramex_gui::app::ParamExApp;
use paramex_gui::format_ui::{DASH, LOW_R2_MESSAGE};
use paramex_gui::state::{EditBuffers, Workspace};
use paramex_gui::workspaces::modelfit::models::LEVEL62_INDEX;
use paramex_gui::workspaces::modelfit::state::{
    DeviceInstallOutcome, ModelFitState, PrimaryTransferSource,
};
use paramex_gui::workspaces::modelfit::ModelFitWorkspace;

struct ParamEditApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
}

impl eframe::App for ParamEditApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            paramex_gui::workspaces::modelfit::panels::summary::show_parameters(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
            );
        });
    }
}

fn parameters_harness(state: ModelFitState) -> Harness<'static, ParamEditApp> {
    let app = ParamEditApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(280.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(320.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();
    harness.run();
    harness
}

fn full_parameters_harness(state: ModelFitState) -> Harness<'static, ParamExApp> {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    *app.modelfit_mut() = state;
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1280.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();
    harness.run();
    harness
}

fn text_input_rects<T>(harness: &Harness<'_, T>) -> Vec<egui::Rect> {
    harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|node| node.rect())
        .collect()
}

fn text_input_for_label<T>(harness: &Harness<'_, T>, label: &str) -> egui::Rect {
    let label_rect = harness.get_by_label(label).rect();
    let input = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .map(|node| node.rect())
        .min_by(|left, right| {
            (left.center().y - label_rect.center().y)
                .abs()
                .total_cmp(&(right.center().y - label_rect.center().y).abs())
        })
        .expect("at least one parameter input");
    assert!(
        (input.center().y - label_rect.center().y).abs() <= 3.0,
        "{label} must share a row with its numeric input: label={label_rect:?}, input={input:?}"
    );
    input
}

fn level62_no_fit_state() -> ModelFitState {
    let mut state = ModelFitState::default();
    let params = ModelParams {
        vt: 2.0,
        gamma: 0.5,
        k: 1.0e-6,
    };
    let vgs: Vec<_> = (0..8).map(|i| i as f64).collect();
    let id = crate::common::modelfit::synthetic_transfer(&params, &vgs);
    let device = crate::common::modelfit::fit_device("short-level62.csv", vgs, id);
    assert_eq!(
        state
            .install_fitted_device(
                device,
                PrimaryTransferSource::new("short-level62.csv", None).unwrap(),
                None,
            )
            .expect("test transfer has no output curves"),
        DeviceInstallOutcome::Installed
    );
    assert!(state
        .selected_entry()
        .is_some_and(|entry| entry.device().level62().is_none()));
    assert!(state.set_selected_model(LEVEL62_INDEX));
    state
}

fn assert_rect_static(label: &str, empty: egui::Rect, loaded: egui::Rect, pixels_per_point: f32) {
    common::assert_same_raster_rect(label, empty, loaded, pixels_per_point);
}

#[test]
fn reset_to_autofit_button_restores_params_and_clears_manual() {
    let mut state = crate::common::modelfit::demo_state(); // "demo: organic" selected, AOSTFT active
    let f0 = *state.selected_entry().unwrap().device().aostft_fit();
    // Enter manual mode with a deliberately wrong VT so the Reset button renders.
    assert!(state.set_selected_fit(f0.vt + 5.0, f0.gamma, f0.k).is_ok());
    assert!(state.is_selected_manual(FitModel::Aostft));

    let mut harness = full_parameters_harness(state);
    let reset_before = harness.get_by_label("Reset to Auto").rect();
    let export_before = harness.get_by_label("Export Verilog-A").rect();
    let first_input_before = text_input_rects(&harness)[0];
    let pixels_per_point = harness.ctx.pixels_per_point();
    common::assert_same_raster_edge(
        "PARAMETERS header and body right rail",
        export_before.right(),
        first_input_before.right(),
        pixels_per_point,
    );
    common::assert_same_raster_edge(
        "PARAMETERS action-row top",
        reset_before.top(),
        export_before.top(),
        pixels_per_point,
    );
    common::assert_same_raster_edge(
        "PARAMETERS action-row bottom",
        reset_before.bottom(),
        export_before.bottom(),
        pixels_per_point,
    );
    assert!(
        reset_before.right() < export_before.left()
            && reset_before.bottom() < first_input_before.top(),
        "PARAMETERS actions should share one title rail with Reset left of Export: reset={reset_before:?}, export={export_before:?}, first_input={first_input_before:?}"
    );

    // The stable action becomes enabled in manual mode; clicking it re-fits.
    harness.get_by_label("Reset to Auto").click();
    common::modelfit::run_until_model_worker_finishes(&mut harness);

    let reset_after = harness.get_by_label("Reset to Auto");
    assert!(
        reset_after.accesskit_node().is_disabled(),
        "Reset stays rendered but becomes disabled in auto mode"
    );
    assert_rect_static(
        "Reset to Auto",
        reset_before,
        reset_after.rect(),
        pixels_per_point,
    );
    assert_rect_static(
        "Export Verilog-A",
        export_before,
        harness.get_by_label("Export Verilog-A").rect(),
        pixels_per_point,
    );
    assert_rect_static(
        "first parameter input",
        first_input_before,
        text_input_rects(&harness)[0],
        pixels_per_point,
    );

    let st = harness.state().modelfit();
    assert!(
        !st.is_selected_manual(FitModel::Aostft),
        "Reset clears manual mode"
    );
    let reset_fit = st.selected_entry().unwrap().device().aostft_fit();
    assert!(
        (reset_fit.vt - f0.vt).abs() < 1e-6,
        "Reset re-extracts the original VT: initial={}, reset={}",
        f0.vt,
        reset_fit.vt,
    );
}

#[test]
fn empty_parameters_card_uses_dash_placeholders() {
    let harness = parameters_harness(ModelFitState::default());

    let dash_count = harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .filter(|node| node.accesskit_node().value().as_deref() == Some(DASH))
        .count();
    assert_eq!(
        dash_count, 12,
        "every empty AOSTFT parameter and device input should use the shared dash placeholder"
    );
}

#[test]
fn parameter_inputs_stay_static_between_empty_and_loaded() {
    let empty = parameters_harness(ModelFitState::default());
    let loaded_state = crate::common::modelfit::demo_state();
    let loaded = parameters_harness(loaded_state);

    let empty_rects = text_input_rects(&empty);
    let loaded_rects = text_input_rects(&loaded);
    assert_eq!(
        empty_rects.len(),
        loaded_rects.len(),
        "PARAMETERS input count should stay fixed between empty and loaded states"
    );
    let pixels_per_point = empty.ctx.pixels_per_point();
    for (idx, (empty, loaded)) in empty_rects.iter().zip(&loaded_rects).enumerate() {
        assert_rect_static(
            &format!("PARAMETERS input {idx}"),
            *empty,
            *loaded,
            pixels_per_point,
        );
    }
}

#[test]
fn parameter_context_and_group_chrome_stay_static_between_states() {
    let empty = parameters_harness(ModelFitState::default());
    let loaded_state = crate::common::modelfit::demo_state();
    let loaded = parameters_harness(loaded_state);
    let pixels_per_point = empty.ctx.pixels_per_point();

    for label in ["channel", "transfer R2", "Model parameters", "Device setup"] {
        assert_rect_static(
            label,
            empty.get_by_label(label).rect(),
            loaded.get_by_label(label).rect(),
            pixels_per_point,
        );
    }

    let channel = loaded.get_by_label("channel").rect();
    let r2 = loaded.get_by_label("transfer R2").rect();
    common::assert_same_raster_edge(
        "PARAMETERS metadata rail top",
        channel.top(),
        r2.top(),
        pixels_per_point,
    );
    common::assert_same_raster_edge(
        "PARAMETERS metadata rail bottom",
        channel.bottom(),
        r2.bottom(),
        pixels_per_point,
    );
    assert!(
        channel.right() < r2.left(),
        "channel and transfer R2 should occupy separate cells on one rail"
    );
}

#[test]
fn level62_inputs_keep_the_same_skeleton_when_empty_loaded_or_unfitted() {
    let mut empty_state = ModelFitState::default();
    assert!(empty_state.set_selected_model(LEVEL62_INDEX));
    let empty = parameters_harness(empty_state);

    let mut loaded_state = crate::common::modelfit::demo_state();
    assert!(loaded_state.set_selected_model(LEVEL62_INDEX));
    let loaded = parameters_harness(loaded_state);
    let no_fit = parameters_harness(level62_no_fit_state());

    let empty_rects = text_input_rects(&empty);
    let loaded_rects = text_input_rects(&loaded);
    let no_fit_rects = text_input_rects(&no_fit);
    assert_eq!(
        empty_rects.len(),
        14,
        "Level 62 should reserve ten model and four device inputs"
    );
    assert_eq!(empty_rects.len(), loaded_rects.len());
    assert_eq!(empty_rects.len(), no_fit_rects.len());
    let pixels_per_point = empty.ctx.pixels_per_point();
    for (idx, (empty, loaded)) in empty_rects.iter().zip(&loaded_rects).enumerate() {
        assert_rect_static(
            &format!("Level 62 input {idx}"),
            *empty,
            *loaded,
            pixels_per_point,
        );
    }
    assert_eq!(
        no_fit
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .filter(|node| node.accesskit_node().value().as_deref() == Some(DASH))
            .count(),
        10,
        "a failed Level 62 extraction should keep its ten model fields as disabled placeholders"
    );
}

#[test]
fn expanded_level62_inputs_scroll_without_moving_device_setup() {
    let mut state = crate::common::modelfit::demo_state();
    assert!(state.set_selected_model(LEVEL62_INDEX));
    let mut params = *state
        .selected_entry()
        .and_then(|entry| entry.device().level62())
        .map(|fit| &fit.params)
        .expect("demo device has a Level 62 fit");
    params.vto += 20.0;
    state
        .set_selected_level62_params(params)
        .expect("deliberately poor manual parameters remain valid");
    let warning_label = LOW_R2_MESSAGE;
    let mut harness = parameters_harness(state);

    let pixels_per_point = harness.ctx.pixels_per_point();
    let vto_before = text_input_for_label(&harness, "VTO (V)");
    let advanced_before = harness.get_by_label("Advanced / constants").rect();
    let warning_before = harness.get_by_label(warning_label).rect();
    let device_before = harness.get_by_label("Device setup").rect();
    let device_labels = ["W (µm)", "L (µm)", "VDS (V)", "Cox (nF/cm²)"];
    let device_inputs_before: Vec<_> = device_labels
        .iter()
        .map(|label| text_input_for_label(&harness, label))
        .collect();

    harness.get_by_label("Advanced / constants").click();
    harness.run();

    let rects = text_input_rects(&harness);
    let reference = rects[0];
    let vto_after = text_input_for_label(&harness, "VTO (V)");
    let advanced_after = harness.get_by_label("Advanced / constants").rect();
    let warning_after = harness.get_by_label(warning_label).rect();
    let vkink = harness.get_by_label("VKINK (V)").rect();
    let device_after_expand = harness.get_by_label("Device setup").rect();
    let device_inputs_after_expand: Vec<_> = device_labels
        .iter()
        .map(|label| text_input_for_label(&harness, label))
        .collect();
    for (label, before, after) in [
        ("Level 62 VTO after expand", vto_before, vto_after),
        (
            "Level 62 Advanced header after expand",
            advanced_before,
            advanced_after,
        ),
        (
            "Level 62 warning after expand",
            warning_before,
            warning_after,
        ),
        (
            "Device setup rail after expand",
            device_before,
            device_after_expand,
        ),
    ] {
        common::assert_same_raster_rect(label, before, after, pixels_per_point);
    }
    for (idx, (before, after)) in device_inputs_before
        .iter()
        .zip(&device_inputs_after_expand)
        .enumerate()
    {
        common::assert_same_raster_rect(
            &format!("Device setup input {idx} after expand"),
            *before,
            *after,
            pixels_per_point,
        );
    }
    assert!(
        advanced_after.bottom() <= warning_after.top() && warning_after.bottom() <= vkink.top(),
        "expanded Level 62 must keep its warning visible between the Advanced header and fields: advanced={advanced_after:?}, warning={warning_after:?}, VKINK={vkink:?}"
    );
    assert!(
        rects.len() > 14,
        "expanded Level 62 should expose advanced fields"
    );
    for (idx, rect) in rects.iter().copied().enumerate() {
        common::assert_same_raster_edge(
            &format!("Level 62 input {idx} left grid edge"),
            rect.left(),
            reference.left(),
            pixels_per_point,
        );
        common::assert_same_raster_edge(
            &format!("Level 62 input {idx} right grid edge"),
            rect.right(),
            reference.right(),
            pixels_per_point,
        );
    }
    let vto = text_input_for_label(&harness, "VTO (V)");
    let vto_order = rects
        .iter()
        .position(|rect| *rect == vto)
        .expect("VTO input");
    let w_order = rects
        .iter()
        .position(|rect| *rect == device_inputs_before[0])
        .expect("W input");
    assert!(
        vto_order < w_order,
        "focus/accessibility order must follow the visual model-before-device order"
    );

    harness.get_by_label("LASAT (m)").scroll_to_me();
    harness.run();
    harness.run();

    let device_after = harness.get_by_label("Device setup").rect();
    common::assert_same_raster_rect(
        "Device setup rail after model-parameter scroll",
        device_before,
        device_after,
        pixels_per_point,
    );
    let device_inputs_after: Vec<_> = device_labels
        .iter()
        .map(|label| text_input_for_label(&harness, label))
        .collect();
    assert_eq!(device_inputs_before.len(), device_inputs_after.len());
    for (idx, (before, after)) in device_inputs_before
        .into_iter()
        .zip(device_inputs_after)
        .enumerate()
    {
        common::assert_same_raster_rect(
            &format!("Device setup input {idx} after model-parameter scroll"),
            before,
            after,
            pixels_per_point,
        );
    }
    let lasat_after = harness.get_by_label("LASAT (m)").rect();
    assert!(
        lasat_after.bottom() < device_after.top(),
        "the scrolled LASAT row must remain above Device setup"
    );
}

// NOTE on the positive type→blur→commit path (audited gap): driving a real keystroke into a
// live param `TextEdit` is not reliably simulable in egui_kittest 0.34 — neither accesskit
// `focus()` nor a simulated `click()` puts the widget into the cursor/editing state that
// consumes `Event::Text`, so a typed value never reaches the buffer (this is why the whole
// suite tests commit handlers + the `commit_only_if_changed` gate + button clicks, and never
// types). The commit path is therefore covered piecewise: the gate by `edit_buffers`, the
// AOSTFT handlers (`set_aostft_fit`/`set_aostft_output`/`set_aostft_subthreshold`) by the
// `summary` unit tests, and the field-render + manual-mode wiring by the reset test above.
