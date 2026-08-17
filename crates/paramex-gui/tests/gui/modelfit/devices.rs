//! Pointer-driven guard for the hand-painted DEVICES selection row: a row uses a
//! painter frame + `ui.interact` (not a stock widget), so a green snapshot can't
//! prove the click commits. Press the row, then assert the committed selection.

use crate::common::modelfit::run_until_model_worker_finishes;
use eframe::egui;
use egui_kittest::{
    kittest::{NodeT, Queryable},
    Harness,
};
use egui_notify::Toasts;
use paramex_core::modelfit::{FitModel, OutputCurve};
use paramex_core::transfer::Session;
use paramex_gui::app::ParamExApp;
use paramex_gui::format_ui::{fmt_num3, LOW_R2_MESSAGE, LOW_R2_THRESHOLD};
use paramex_gui::state::{EditBuffers, Workspace};
use paramex_gui::workspaces::modelfit::models::LEVEL62_INDEX;
use paramex_gui::workspaces::modelfit::state::{
    DeviceInstallOutcome, ModelFitState, OutputSource, PrimaryTransferSource,
};
use paramex_gui::workspaces::modelfit::ModelFitWorkspace;

struct DevicesHarnessApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
    actions_enabled: bool,
}

fn full_model_harness(state: ModelFitState) -> Harness<'static, ParamExApp> {
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

impl eframe::App for DevicesHarnessApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            paramex_gui::workspaces::modelfit::panels::devices::show(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
                self.actions_enabled,
            );
        });
    }
}

struct DevicesAndParamsHarnessApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
}

impl eframe::App for DevicesAndParamsHarnessApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            ui.columns(2, |cols| {
                paramex_gui::workspaces::modelfit::panels::devices::show(
                    &mut cols[0],
                    &mut self.workspace,
                    &mut self.edits,
                    &mut self.toasts,
                    true,
                );
                paramex_gui::workspaces::modelfit::panels::summary::show_parameters(
                    &mut cols[1],
                    &mut self.workspace,
                    &mut self.edits,
                    &mut self.toasts,
                );
            });
        });
    }
}

#[test]
fn clicking_a_device_row_updates_selection() {
    let state = crate::common::modelfit::demo_state();
    // The first device is selected by default; target a different one.
    let target = "demo: LTPS";
    assert_ne!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some(target),
        "target must start unselected"
    );

    let app = DevicesHarnessApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(320.0, 420.0),
        actions_enabled: true,
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(360.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    harness.get_by_label(target).click();
    harness.run();

    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some(target),
        "clicking the row should commit the selection"
    );
}

#[test]
fn device_rows_expose_radio_semantics_and_keyboard_selection() {
    let state = crate::common::modelfit::demo_state();
    let selected = "demo: organic";
    let target = "demo: LTPS";
    let app = DevicesHarnessApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(320.0, 420.0),
        actions_enabled: true,
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(360.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    let selected_label = format!("Select {selected}");
    let target_label = format!("Select {target}");
    {
        let selected_row =
            harness.get_by_role_and_label(egui::accesskit::Role::RadioButton, &selected_label);
        assert_eq!(
            selected_row.accesskit_node().toggled(),
            Some(egui::accesskit::Toggled::True)
        );
        let target_row =
            harness.get_by_role_and_label(egui::accesskit::Role::RadioButton, &target_label);
        assert_eq!(
            target_row.accesskit_node().toggled(),
            Some(egui::accesskit::Toggled::False)
        );
        target_row.focus();
    }
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    harness.run();
    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some(target)
    );
}

#[test]
fn low_active_model_fit_stays_on_the_selected_device_and_shows_a_warning() {
    let mut state = crate::common::modelfit::demo_state();
    assert_eq!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some("demo: organic")
    );
    assert!(state.set_selected_model(LEVEL62_INDEX));

    let mut poor = state
        .selected_entry()
        .and_then(|entry| entry.device().level62())
        .expect("selected demo has a Level 62 fit")
        .params;
    poor.vto += 100.0;
    assert!(state.set_selected_level62_params(poor).is_ok());
    assert!(
        state
            .selected_entry()
            .and_then(|entry| entry.device().model(FitModel::Level62).r2())
            .is_some_and(|r2| r2 < LOW_R2_THRESHOLD),
        "test setup must create a real low-R² active model fit"
    );
    let transfer_r2 = state
        .selected_entry()
        .and_then(|entry| entry.device().model(FitModel::Level62).r2())
        .expect("active model has transfer fit quality");
    let row_quality = format!("transfer R\u{00B2} {}", fmt_num3(transfer_r2));

    let app = DevicesAndParamsHarnessApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(680.0, 420.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(720.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some("demo: organic"),
        "switching to a poor active-model fit keeps the original device selected"
    );
    harness.get_by_label("demo: organic");
    harness.get_by_label(&row_quality);
    harness.get_by_label("Low R\u{00B2}; review fit range.");
    assert!(
        harness.get_all_by_label("WARN").count() >= 2,
        "the low-R\u{00B2} device row and PARAMETERS status should both warn"
    );
}

#[test]
fn full_overlay_r2_takes_precedence_for_the_bad_real_fit() {
    let mut state = ModelFitState::default();
    crate::common::modelfit::install_fixture(&mut state, "4-1.xlsx");
    assert!(state.set_selected_model(LEVEL62_INDEX));
    let quality = state
        .selected_entry()
        .expect("selected device")
        .device()
        .model(FitModel::Level62)
        .analog_fit_quality();
    assert!(
        quality.gm_p90.is_some_and(|error| error >= 0.15),
        "fixture must retain its analog mismatch"
    );
    let r2 = state
        .selected_entry()
        .expect("selected device")
        .device()
        .model(FitModel::Level62)
        .r2()
        .expect("Level 62 R2");
    assert!(
        r2 < LOW_R2_THRESHOLD,
        "the full displayed overlay must expose the poor global fit"
    );
    let row_warning = format!("transfer R\u{00B2} {}", fmt_num3(r2));

    let app = DevicesAndParamsHarnessApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(680.0, 420.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(720.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    harness.get_by_label(&row_warning);
    harness.get_by_label(LOW_R2_MESSAGE);
    assert_eq!(
        harness.get_all_by_label("WARN").count(),
        2,
        "both the device row and parameter detail must warn for the poor full overlay"
    );
}

#[test]
fn busy_device_rows_keep_selection_and_checkboxes_active_but_disable_destructive_actions() {
    let state = crate::common::modelfit::demo_state();
    assert_eq!(
        state.selected_entry().and_then(|entry| entry.output_name()),
        Some("demo: organic_output.xlsx")
    );
    let target = "demo: LTPS";
    assert_ne!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some(target)
    );

    let app = DevicesHarnessApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(320.0, 420.0),
        actions_enabled: false,
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(360.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    harness.get_by_label(target).click();
    harness.run();

    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some(target),
        "busy-state gating should not disable harmless row selection"
    );
    for label in ["Remove Selected", "Clear All", "Keep Checked"] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} is disabled while Model Fit IO is running"
        );
    }
    for label in ["Remove attached output", "Detach output"] {
        assert!(
            harness.get_by_label(label).accesskit_node().is_disabled(),
            "{label} is disabled while Model Fit IO is running"
        );
    }
    harness
        .get_by_role_and_label(
            egui::accesskit::Role::CheckBox,
            "Mark demo: organic for bulk actions",
        )
        .click_accesskit();
    harness.run();
    assert!(harness.state().workspace.state().has_checked_devices());
    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some(target),
        "bulk checking does not steal the active row selection"
    );
}

#[test]
fn model_fit_parse_error_row_is_visible_and_dismissible() {
    let mut workspace = ModelFitWorkspace::default();
    workspace.record_ingest_error("bad_model.csv".into(), "missing Vg column".into());
    let app = DevicesHarnessApp {
        workspace,
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(320.0, 420.0),
        actions_enabled: true,
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(360.0, 460.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    harness.get_by_label("bad_model.csv");
    harness.get_by_label("Dismiss").click();
    harness.run();

    assert!(
        !harness.state().workspace.has_ingest_errors(),
        "dismiss removes the persistent Model Fit error row"
    );
}

#[test]
fn attached_output_file_is_visible_below_its_transfer_device() {
    let mut state = ModelFitState::default();
    let mut device = crate::common::modelfit::fit_device(
        "B_1.xlsx",
        vec![-1.0, 0.0, 1.0, 2.0, 3.0, 4.0],
        vec![1.0e-12, 2.0e-12, 1.0e-9, 1.0e-6, 4.0e-6, 9.0e-6],
    );
    let curves = vec![OutputCurve {
        vg: 4.0,
        vds: vec![0.0, 1.0, 2.0, 3.0],
        id: vec![0.0, 1.0e-6, 1.8e-6, 2.4e-6],
    }];
    assert!(device
        .replace_output(curves)
        .expect("device without retained DIBL accepts output")
        .displaced
        .is_empty());
    assert_eq!(
        state
            .install_fitted_device(
                device,
                PrimaryTransferSource::new("B_1.xlsx", None).unwrap(),
                Some(OutputSource::new("B_1_output.xlsx", None).unwrap()),
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );

    let mut harness = full_model_harness(state);

    let transfer_left = harness.get_by_label("B_1.xlsx").rect().left();
    let output_left = harness.get_by_label("B_1_output.xlsx").rect().left();
    assert_eq!(
        transfer_left, output_left,
        "Model Fit files should share one flat filename column"
    );
    harness.get_by_label("attached");

    harness.get_by_label("Remove attached output").click();
    run_until_model_worker_finishes(&mut harness);

    assert!(harness.query_by_label("B_1_output.xlsx").is_none());
    let entry = harness
        .state()
        .modelfit()
        .selected_entry()
        .expect("transfer remains");
    assert_eq!(entry.output_name(), None);
    assert!(entry.device().output().is_none());
    assert!(!entry.device().has_output_curves());
}

#[test]
fn attached_output_can_detach_to_pending_and_reattach() {
    let mut state = ModelFitState::default();
    let mut device = crate::common::modelfit::fit_device(
        "B_1.xlsx",
        vec![-1.0, 0.0, 1.0, 2.0, 3.0, 4.0],
        vec![1.0e-12, 2.0e-12, 1.0e-9, 1.0e-6, 4.0e-6, 9.0e-6],
    );
    let curves = vec![OutputCurve {
        vg: 4.0,
        vds: vec![0.0, 1.0, 2.0, 3.0],
        id: vec![0.0, 1.0e-6, 1.8e-6, 2.4e-6],
    }];
    assert!(device
        .replace_output(curves)
        .expect("device without retained DIBL accepts output")
        .displaced
        .is_empty());
    assert_eq!(
        state
            .install_fitted_device(
                device,
                PrimaryTransferSource::new("B_1.xlsx", None).unwrap(),
                Some(OutputSource::new("B_1_output.xlsx", None).unwrap()),
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );

    let mut harness = full_model_harness(state);

    harness.get_by_label("Detach output").click();
    run_until_model_worker_finishes(&mut harness);
    assert_eq!(
        harness
            .state()
            .modelfit()
            .selected_entry()
            .unwrap()
            .output_name(),
        None,
        "detaching clears the selected device's output"
    );
    harness.get_by_label("B_1_output.xlsx");
    harness.get_by_label("Detached");
    harness.get_by_label("Attach to Selected").click();
    run_until_model_worker_finishes(&mut harness);
    assert_eq!(
        harness
            .state()
            .modelfit()
            .selected_entry()
            .unwrap()
            .output_name(),
        Some("B_1_output.xlsx"),
        "reattaching restores the pending output on the selected device"
    );
}
