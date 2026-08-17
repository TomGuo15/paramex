use egui_kittest::{kittest::Queryable, Harness};
use paramex_core::modelfit::{ModelParams, OutputCurve, SubthresholdParams};
use paramex_gui::workspaces::modelfit::{
    models::LEVEL62_INDEX,
    panels::{gds_plot, output_plot},
    state::{DeviceInstallOutcome, ModelFitState, OutputSource, PrimaryTransferSource},
};

struct OutputPlotHarness {
    state: ModelFitState,
}

impl eframe::App for OutputPlotHarness {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(eframe::egui::Vec2::new(330.0, 230.0), |ui| {
            output_plot::show(ui, &self.state);
        });
    }
}

struct OutputPlotsHarness {
    state: ModelFitState,
}

impl eframe::App for OutputPlotsHarness {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        ui.allocate_ui(eframe::egui::Vec2::new(360.0, 250.0), |ui| {
            output_plot::show(ui, &self.state);
        });
        ui.allocate_ui(eframe::egui::Vec2::new(360.0, 250.0), |ui| {
            gds_plot::show(ui, &self.state);
        });
    }
}

#[test]
fn output_family_headers_explain_gate_color_order() {
    let state = crate::common::modelfit::demo_state();
    let mut harness = Harness::builder()
        .with_size(eframe::egui::Vec2::new(380.0, 540.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            OutputPlotsHarness { state }
        });
    harness.run();

    assert_eq!(
        harness.get_all_by_label("VG 0 \u{2192} -5 V").count(),
        2,
        "both output-family cards should explain their pale-to-vivid gate-voltage order"
    );
}

fn failed_output_state() -> ModelFitState {
    let mut state = ModelFitState::default();
    let vg: Vec<f64> = (0..=120).map(|i| -2.0 + i as f64 * 0.1).collect();
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let id = crate::common::modelfit::synthetic_unified_transfer(3.0, 0.5, 1.0e-6, &sub, &vg);
    let mut device = crate::common::modelfit::fit_device("B_1.xlsx", vg, id);
    let vds: Vec<f64> = (0..=30).map(|i| i as f64 * 0.5).collect();
    let curves: Vec<_> = [0.0, 1.0, 2.0]
        .into_iter()
        .map(|vg| OutputCurve {
            vg,
            vds: vds.clone(),
            id: vec![1.0e-6; vds.len()],
        })
        .collect();
    assert!(device
        .replace_output(curves)
        .expect("device without retained DIBL accepts output")
        .displaced
        .is_empty());
    assert!(device.has_output_curves());
    assert!(!device.has_output());
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
    state
}

#[test]
fn no_device_output_tile_stays_neutral() {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::Vec2::new(360.0, 260.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            OutputPlotHarness {
                state: ModelFitState::default(),
            }
        });
    harness.run();

    harness.get_by_label("OUTPUT FIT");
    assert!(harness.query_by_label("EMPTY").is_none());
    assert!(
        harness.query_by_label("No output fit series.").is_none(),
        "cold-start OUTPUT FIT should keep the reserved plot and footer neutral"
    );
}

#[test]
fn loaded_device_without_output_series_keeps_compact_status_copy() {
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
    assert!(state.selected_entry().is_some_and(|entry| {
        entry
            .device()
            .model(state.selected_fit_model())
            .output_family()
            .is_empty()
    }));

    let mut harness = Harness::builder()
        .with_size(eframe::egui::Vec2::new(360.0, 260.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            OutputPlotHarness { state }
        });
    harness.run();

    harness.get_by_label("EMPTY");
    harness.get_by_label("No output fit series.");
}

#[test]
fn failed_output_fit_keeps_measured_legend_and_warns_both_output_tiles() {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::Vec2::new(380.0, 540.0))
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            OutputPlotsHarness {
                state: failed_output_state(),
            }
        });
    harness.run();

    harness.get_by_label("MEASURED");
    harness.get_by_label("measured");
    assert!(harness.query_by_label("model").is_none());
    let measured = harness.get_by_label("MEASURED").rect();
    let gds_top = harness.get_by_label("OUTPUT CONDUCTANCE").rect().top();
    assert!(
        measured.bottom() <= gds_top,
        "the measured-only status must remain inside OUTPUT FIT: measured={measured:?}, next tile top={gds_top}"
    );
    assert_eq!(
        harness.get_all_by_label("Output fit failed.").count(),
        2,
        "both output tiles should explain the failed fit"
    );
}
