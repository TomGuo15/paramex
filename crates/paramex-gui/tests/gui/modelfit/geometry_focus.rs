//! Pointer-driven guard for the W/L geometry edit buffers across a device
//! switch. The buffers commit on focus-loss; a field focused on device A whose
//! focus is stolen by clicking device B's row must NOT commit A's value onto B
//! (the focus-steal / deferred-commit class). Renders devices THEN summary, mirroring the
//! page's left-before-right order so the select + forget runs before the W field.

use eframe::egui;
use egui_kittest::{kittest::Queryable, Harness};
use egui_notify::Toasts;
use paramex_core::modelfit::{BiasParams, FittedDevice};
use paramex_gui::state::EditBuffers;
use paramex_gui::workspaces::modelfit::state::ModelFitState;
use paramex_gui::workspaces::modelfit::ModelFitWorkspace;

struct GeomFocusApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
}

impl eframe::App for GeomFocusApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            // Same order as the real page: the DEVICES list (which commits the
            // selection + forgets stale geometry buffers) renders before the
            // SELECTED DEVICE panel (which renders the W/L fields).
            paramex_gui::workspaces::modelfit::panels::devices::show(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
                true,
            );
            // Render the PARAMETERS card after the DEVICES list, exactly as on
            // the real page.
            paramex_gui::workspaces::modelfit::panels::summary::show_parameters(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
            );
        });
    }
}

fn w_um_of(state: &ModelFitState, name: &str) -> f64 {
    device(state, name).geometry().w_um
}

fn cox_of(state: &ModelFitState, name: &str) -> f64 {
    device(state, name).bias().cox
}

fn vt_of(state: &ModelFitState, name: &str) -> f64 {
    device(state, name).aostft_fit().vt
}

fn device<'a>(state: &'a ModelFitState, name: &str) -> &'a FittedDevice {
    state
        .devices()
        .iter()
        .find(|entry| entry.device().name() == name)
        .map(|entry| entry.device())
        .unwrap_or_else(|| panic!("device {name} present"))
}

#[test]
fn switching_device_while_a_parameter_field_is_focused_does_not_corrupt_the_new_device() {
    let mut state = crate::common::modelfit::demo_state(); // organic selected, LTPS second
    let organic_fit = *state.selected_entry().unwrap().device().aostft_fit();
    assert!(state
        .set_selected_fit(organic_fit.vt + 5.0, organic_fit.gamma, organic_fit.k)
        .is_ok());
    let organic_vt = vt_of(&state, "demo: organic");
    let ltps_vt = vt_of(&state, "demo: LTPS");
    assert_ne!(organic_vt, ltps_vt);

    let app = GeomFocusApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(1280.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1320.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    // Focus VTH: it is the first editable parameter input above the geometry rows.
    {
        let vt = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .min_by(|a, b| a.rect().center().y.total_cmp(&b.rect().center().y))
            .expect("a parameter text input is present");
        vt.focus();
    }
    harness.run();

    harness.get_by_label("demo: LTPS").click();
    harness.run();

    assert_eq!(
        vt_of(harness.state().workspace.state(), "demo: LTPS"),
        ltps_vt,
        "switching devices must not commit the focused parameter onto the new device"
    );
    assert_eq!(
        vt_of(harness.state().workspace.state(), "demo: organic"),
        organic_vt,
        "the original device's parameter must be untouched"
    );
}

#[test]
fn switching_device_while_a_geometry_field_is_focused_does_not_corrupt_the_new_device() {
    let state = crate::common::modelfit::demo_state_with("demo: organic", |device| {
        device
            .set_geometry(paramex_core::modelfit::GeometryParams {
                w_um: 55.0,
                l_um: 10.0,
            })
            .unwrap();
    });
    assert_eq!(w_um_of(&state, "demo: organic"), 55.0);
    assert_eq!(w_um_of(&state, "demo: LTPS"), 1500.0);

    let app = GeomFocusApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(1280.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1320.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    // Focus organic's W field (the upper of the two geometry inputs — W row sits
    // above L row). Its buffer now holds organic's "55.000".
    {
        let w = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .min_by(|a, b| a.rect().center().y.total_cmp(&b.rect().center().y))
            .expect("a W/L text input is present");
        w.focus();
    }
    harness.run();

    // Click a DIFFERENT device's row, stealing focus from the W field.
    harness.get_by_label("demo: LTPS").click();
    harness.run();

    // The stolen-focus commit must NOT write organic's 55 onto LTPS.
    assert_eq!(
        w_um_of(harness.state().workspace.state(), "demo: LTPS"),
        1500.0,
        "switching devices must not commit the focused field's value onto the new device"
    );
    assert_eq!(
        w_um_of(harness.state().workspace.state(), "demo: organic"),
        55.0,
        "the original device's geometry must be untouched"
    );
}

#[test]
fn switching_device_while_the_cox_field_is_focused_does_not_corrupt_the_new_device() {
    // Same focus-steal class for the Cox field, which uses a DISTINCT round-trip
    // formatter (scientific, not 3-decimal) — guard it independently.
    let state = crate::common::modelfit::demo_state_with("demo: organic", |device| {
        device.set_bias(0.1, 1.0e-3).unwrap();
    });
    let default_cox = BiasParams::default().cox;
    assert_eq!(cox_of(&state, "demo: organic"), 1.0e-3);
    assert_eq!(cox_of(&state, "demo: LTPS"), default_cox);

    let app = GeomFocusApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(1280.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1320.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    // Focus organic's Cox field — the LAST of the four inputs (W, L, V_DS, Cox top
    // to bottom), so the lowest-y text input.
    {
        let cox = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .max_by(|a, b| a.rect().center().y.total_cmp(&b.rect().center().y))
            .expect("a bias text input is present");
        cox.focus();
    }
    harness.run();

    harness.get_by_label("demo: LTPS").click();
    harness.run();

    assert_eq!(
        cox_of(harness.state().workspace.state(), "demo: LTPS"),
        default_cox,
        "switching devices must not commit the focused Cox onto the new device"
    );
    assert_eq!(
        cox_of(harness.state().workspace.state(), "demo: organic"),
        1.0e-3,
        "the original device's Cox must be untouched"
    );
}

/// Harness for the model-switch focus-steal path: the DATA card (the model
/// dropdown) renders BEFORE the PARAMETERS card (the W/L fields), mirroring the
/// page's left-before-right order.
struct ModelSwitchApp {
    workspace: ModelFitWorkspace,
    edits: EditBuffers,
    toasts: Toasts,
    size: egui::Vec2,
}

impl eframe::App for ModelSwitchApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(self.size, |ui| {
            paramex_gui::workspaces::modelfit::panels::inputs::show(
                ui,
                &mut self.workspace,
                &mut self.edits,
            );
            paramex_gui::workspaces::modelfit::panels::summary::show_parameters(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
            );
        });
    }
}

#[test]
fn switching_models_while_a_geometry_field_is_focused_does_not_corrupt_the_selected_device() {
    // The model-switch arm of the focus-steal class: the real ComboBox is multi-frame — it
    // steals a focused geometry field's focus on popup-OPEN (committing the field against the
    // still-selected device) before the pick. A regression to a single-frame picker, or a
    // commit routed to the wrong device, would corrupt the selected device's geometry on a
    // model switch. This guard renders that flow and asserts the model switches and the
    // focused device's W is untouched.
    let mut state = crate::common::modelfit::demo_state_with("demo: LTPS", |device| {
        device
            .set_geometry(paramex_core::modelfit::GeometryParams {
                w_um: 55.0,
                l_um: 10.0,
            })
            .unwrap();
    }); // organic (0, AOSTFT home), LTPS (1, Level 62 home)
    let ltps = state
        .devices()
        .iter()
        .position(|entry| entry.device().name() == "demo: LTPS")
        .unwrap();
    state.select(ltps);
    assert_eq!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some("demo: LTPS")
    );
    // LTPS has a DISTINCT W so any stray commit is visible.
    assert_eq!(w_um_of(&state, "demo: LTPS"), 55.0);

    let app = ModelSwitchApp {
        workspace: ModelFitWorkspace::from_state(state),
        edits: EditBuffers::default(),
        toasts: Toasts::default(),
        size: egui::Vec2::new(1280.0, 760.0),
    };
    let mut harness = Harness::builder()
        .with_size(egui::Vec2::new(1320.0, 800.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();

    // Focus the selected device's W field (the upper of the two geometry inputs).
    {
        let w = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .min_by(|a, b| a.rect().center().y.total_cmp(&b.rect().center().y))
            .expect("a W/L text input is present");
        w.focus();
    }
    harness.run();

    // Open the model dropdown (steals the W field's focus) and pick Level 62. LTPS fits Level 62
    // well, so the selection stays on LTPS — the commit on focus-steal must route back to LTPS,
    // not lose or corrupt its W. The ComboBox exposes its selected text as the accesskit
    // `value`, so open it by role; the popup options render as labeled rows.
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .next()
        .expect("model dropdown is present")
        .click();
    harness.run();
    harness.get_by_label("Level 62 / LTPS").click();
    harness.run();

    // The model switched and the selection stayed on LTPS (a well-fitting device).
    assert_eq!(
        harness.state().workspace.state().selected_model(),
        1,
        "Level 62 is selected"
    );
    assert_eq!(
        harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .map(|entry| entry.device().name()),
        Some("demo: LTPS"),
        "a well-fitting device stays selected across the switch"
    );
    // The focus-steal must NOT have corrupted the selected device's geometry.
    assert_eq!(
        w_um_of(harness.state().workspace.state(), "demo: LTPS"),
        55.0,
        "switching models must not corrupt the focused device's geometry"
    );
}
