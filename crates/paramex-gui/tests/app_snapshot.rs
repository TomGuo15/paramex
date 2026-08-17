//! Headless full-app render via wgpu. This is the capability that was missing for
//! 6b/6c/6d: the real `ParamExApp` layout rendered to a PNG so visual problems
//! (fonts, theme, layout) are actually visible, not deferred to a manual smoke.
//!
//! Run: `cargo test -p paramex-gui --test app_snapshot`. The rendered PNGs land in
//! `crates/paramex-gui/tests/snapshots/` (created on first run).

mod common;

use common::loaded_tlm_app as seed_tlm_app;
use egui_kittest::kittest::NodeT;
use egui_kittest::{kittest::Queryable, Harness};
use paramex_core::modelfit::{FitModel, OutputCurve, SubthresholdParams};
use paramex_core::transfer::{OutputCurve as TransferOutputCurve, OutputDataset, Session};
use paramex_gui::app::ParamExApp;
use paramex_gui::state::Workspace;
use paramex_gui::workspaces::modelfit::state::{
    DeviceInstallOutcome, OutputSource, PrimaryTransferSource,
};
use std::sync::Mutex;

static HARNESS_LOCK: Mutex<()> = Mutex::new(());

const DOUBLE: &str =
    include_str!("../../paramex-core/tests/reference/parse/fixtures/corpus_double.csv");
const SINGLE: &str =
    include_str!("../../paramex-core/tests/reference/parse/fixtures/corpus_single_a.csv");

fn painted_text_rects(
    shape: &eframe::egui::epaint::Shape,
    needle: &str,
    rects: &mut Vec<egui::Rect>,
) {
    match shape {
        eframe::egui::epaint::Shape::Text(text) if text.galley.job.text == needle => {
            rects.push(text.visual_bounding_rect());
        }
        eframe::egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                painted_text_rects(shape, needle, rects);
            }
        }
        _ => {}
    }
}

fn painted_text_count(shape: &eframe::egui::epaint::Shape, needle: &str) -> usize {
    let mut rects = Vec::new();
    painted_text_rects(shape, needle, &mut rects);
    rects.len()
}

/// A realistic app state: two loaded files (one selected, double-sweep) plus one
/// ingestion-error row — so file rows, the selected style, the error row, the
/// metric tiles, the results table, and the selector are all on screen.
fn seed_app() -> ParamExApp {
    let mut s = Session::new();
    let id1 = s
        .add_curve(common::parse_transfer_fixture(DOUBLE, "corpus_double.csv"))
        .unwrap();
    s.add_curve(common::parse_transfer_fixture(
        SINGLE,
        "corpus_single_a.csv",
    ))
    .unwrap();
    assert!(s.select_file(&id1));
    let mut app = ParamExApp::from_session(s);
    app.transfer_mut().record_ingest_error(
        "bad_device.csv".to_string(),
        // The REAL two-sentence parse diagnostic (core::parse), so the scenes
        // exercise the wrapped error row, not a short stand-in.
        "No usable transfer curve found in bad_device.csv. Check that the file \
         contains Vg and Id columns with at least 12 valid positive-current rows."
            .to_string(),
    );
    app
}

fn seed_many_files_app() -> ParamExApp {
    let mut s = Session::new();
    let mut ids = Vec::new();
    for idx in 0..24 {
        let id = s
            .add_curve(common::parse_transfer_fixture(
                DOUBLE,
                &format!("A_{idx:02}.csv"),
            ))
            .unwrap();
        ids.push(id);
    }
    if let Some(first) = ids.first() {
        assert!(s.select_file(first));
    }
    ParamExApp::from_session(s)
}

fn seed_selected_warning_app() -> ParamExApp {
    let mut s = Session::new();
    let id = s
        .add_curve(common::partial_transfer_curve("partial_curve.csv"))
        .unwrap();
    assert!(s.select_file(&id));
    ParamExApp::from_session(s)
}

fn seed_transfer_output_app() -> ParamExApp {
    let mut s = Session::new();
    let id = s
        .add_curve(common::parse_transfer_fixture(DOUBLE, "corpus_double.csv"))
        .unwrap();
    assert!(s.select_file(&id));
    assert!(s
        .replace_output(
            &id,
            OutputDataset {
                name: "corpus_double_output.csv".to_string(),
                curves: vec![
                    TransferOutputCurve {
                        vg: 5.0,
                        vd: vec![0.0, 1.0, 2.0, 3.0],
                        id: vec![0.0, 1.0e-6, 1.7e-6, 2.5e-6],
                    },
                    TransferOutputCurve {
                        vg: 1.0,
                        vd: vec![0.0, 1.0, 2.0, 3.0],
                        id: vec![0.0, 0.2e-6, 0.34e-6, 0.5e-6],
                    },
                    TransferOutputCurve {
                        vg: 3.0,
                        vd: vec![0.0, 1.0, 2.0, 3.0],
                        id: vec![0.0, 0.6e-6, 1.02e-6, 1.5e-6],
                    },
                ],
                source_path: None,
            },
        )
        .is_ok());
    ParamExApp::from_session(s)
}

fn render(name: &str, size: egui::Vec2) {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness_at_size(seed_app(), size);
    assert!(harness.query_by_label("Extraction OK.").is_none());
    harness.snapshot(name);
}

fn transfer_output_harness(size: egui::Vec2, app: ParamExApp) -> Harness<'static, ParamExApp> {
    let mut harness = common::app_harness_at_size(app, size);
    harness.get_by_label("Output Fit").click();
    harness.run();
    harness.run();
    harness
}

#[test]
fn banner_exposes_transfer_as_active_workspace_navigation() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = common::app_harness(seed_app());

    assert!(harness.get_by_label("ParamEx").rect().is_positive());
    // accesskit sees the plain label text: "Transfer"
    assert!(harness.get_by_label("Transfer").rect().is_positive());
    assert!(harness.get_by_label("TLM").rect().is_positive());
    assert!(harness.get_by_label("Data guide").rect().is_positive());
    assert!(harness.query_by_label("?").is_none());
}

#[test]
fn technical_guide_tabs_show_exact_contracts_and_equations() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());

    harness.get_by_label("Data guide").click();
    harness.run();
    harness.run();

    assert!(harness.get_by_label("INPUT").rect().is_positive());
    assert!(harness
        .get_by_label("At least 12 measured points across the gate sweep.")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Hysteresis needs forward + reverse sweeps (≥12 points each).")
        .rect()
        .is_positive());
    assert!(harness.query_by_label("DATA GUIDE").is_none());
    assert!(harness
        .query_by_label("Accepted files and pairing rules for each workspace.")
        .is_none());
    harness.snapshot("app_data_guide");

    harness.get_by_label("TLM guide").click();
    harness.run();
    harness.run();
    assert!(harness
        .get_by_label("Number folder = channel length (μm).")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("List(*) sheet: vg · abs_id · abs_is")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Setup(*) sheet: VD; else Fallback VD.")
        .rect()
        .is_positive());
    assert!(harness.query_by_label("List*").is_none());
    assert!(harness.query_by_label("Setup*").is_none());
    assert!(harness
        .get_by_label(
            "Need ≥2 lengths (≥3 for R2). Primary: highest-current device per L; median: diagnostic. m is slope (Ω/μm), not sheet resistance."
        )
        .rect()
        .is_positive());
    harness.snapshot("app_data_guide_tlm");

    harness.get_by_label("Model Fit guide").click();
    harness.run();
    harness.run();
    assert!(harness
        .get_by_label("Output and C-V files are optional.")
        .rect()
        .is_positive());
    assert!(harness.query_by_label("DIBL").is_none());
    assert!(harness
        .get_by_label("UMEM H-function extraction")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Output-attached card mapping")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Above-/subthreshold transfer crossover")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Finite-VDS channel current")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("The transfer sweep must include the off region.")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label(
            "Value is matched. Transfer-only slope is exact when the radicand ≥ 0; with output data it is approximate."
        )
        .rect()
        .is_positive());
    assert!(harness
        .query_by_label("Transfer + Eq. 28/29 crossover")
        .is_none());
    assert!(harness
        .query_by_label("Finite-VDS Eq. 25 mapping")
        .is_none());
    assert!(harness
        .get_by_label("H of V G equals the integral from the first gate voltage to V G of absolute I D d u, divided by absolute I D of V G, and equals a V G plus b; V T equals minus b over a; gamma H equals one over a minus two; K H is the median of absolute I D divided by V G minus V T to the one plus gamma H power, where current is at least one percent of peak and V G is above V T.")
        .rect()
        .is_positive());
    harness.snapshot("app_data_guide_model_aostft");

    harness
        .get_by_label("Analog terminal charge")
        .scroll_to_me();
    harness.run();
    harness.run();
    harness.snapshot("app_data_guide_model_aostft_lower");

    assert!(harness.get_by_label("Level 62 / LTPS").rect().is_positive());
    assert!(harness
        .query_by_label("Level 62-derived equations")
        .is_none());
    harness.get_by_label("Level 62 / LTPS").click();
    harness.run();
    harness.run();
    assert!(harness
        .get_by_label("Output, DIBL, and C-V files are optional.")
        .rect()
        .is_positive());
    assert!(!harness.get_by_label("DIBL").accesskit_node().is_disabled());
    assert!(harness
        .get_by_label("Leakage + impact-ionization kink")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label(
            "Vth is thermal voltage; VTO is zero-bias threshold. Here Leff = L. Fit uses TNOM; export uses simulator temperature."
        )
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Ikink = 0 until VDS − VDsk exceeds its numerical guard.")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Level 62-derived charge equations: x q equals V G S minus V T O; the displayed DELTA clamp defines V G T E q; u delta of z equals z plus square root of z squared plus delta squared, over two; a equals V G T E q and b equals u zero point zero five volts of a minus V D; Q g and the drain fraction use the displayed Meyer equations with ten to minus nine volt and ten to minus eighteen volt squared guards; Q d and Q s partition minus Q g.")
        .rect()
        .is_positive());
    assert!(harness
        .query_by_label("UMEM H-function extraction")
        .is_none());
    harness.snapshot("app_data_guide_model_level62");

    harness
        .get_by_label("Analog terminal charge")
        .scroll_to_me();
    harness.run();
    harness.run();
    harness.snapshot("app_data_guide_model_level62_lower");

    harness.get_by_label("Close guide").click();
    harness.run();
    harness.run();
    assert!(harness
        .query_by_label("Leakage + impact-ionization kink")
        .is_none());
}

#[test]
fn technical_guide_opens_on_the_active_workspace_and_model() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_level62_app());

    harness.get_by_label("Data guide").click();
    harness.run();
    harness.run();

    assert!(harness
        .get_by_label("Output, DIBL, and C-V files are optional.")
        .rect()
        .is_positive());
    assert!(harness
        .get_by_label("Electrostatics + mobility")
        .rect()
        .is_positive());
    assert!(harness
        .query_by_label("At least 12 measured points across the gate sweep.")
        .is_none());
    assert!(harness
        .query_by_label("UMEM H-function extraction")
        .is_none());
}

#[test]
fn technical_guide_blocks_workspace_interaction_until_closed() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());

    let tlm = harness.get_by_label("TLM").rect().center();
    harness.get_by_label("Data guide").click();
    harness.run();
    harness.run();

    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(tlm));
    for pressed in [true, false] {
        harness.input_mut().events.push(egui::Event::PointerButton {
            pos: tlm,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
    harness.run();

    assert!(
        harness.query_by_label("ANALYSIS").is_none(),
        "the guide must block workspace navigation behind it"
    );
    assert!(
        harness.query_by_label("TECHNICAL GUIDE").is_none(),
        "clicking the modal backdrop should dismiss the guide"
    );
}

/// The app at its real 1280×800 window size — what the user actually sees.
#[test]
fn render_real_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());
    assert!(harness.query_by_label("Extraction OK.").is_none());
    for tick in ["-2", "2", "4"] {
        let count: usize = harness
            .output()
            .shapes
            .iter()
            .map(|shape| painted_text_count(&shape.shape, tick))
            .sum();
        assert_eq!(
            count, 2,
            "both production-width selector plots must paint the {tick} V tick"
        );
    }
    harness.snapshot("app_real");
}

#[test]
fn render_transfer_output_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        transfer_output_harness(egui::Vec2::new(1280.0, 800.0), seed_transfer_output_app());
    let output_labels = harness.get_all_by_label("OUTPUT").count();
    assert!(
        output_labels >= 2,
        "Output Fit should render the OUTPUT panel header in addition to the file-list output badge"
    );
    harness.get_by_label("VG 1 \u{2192} 5 V");
    assert!(harness.get_by_label("Clear All").rect().is_positive());
    let table_top = harness.get_by_label("Output file").rect().bottom();
    for value in ["corpus_double.csv", "corpus_double_output.csv", "Family"] {
        let mut candidates = Vec::new();
        for clipped in &harness.output().shapes {
            let mut rects = Vec::new();
            painted_text_rects(&clipped.shape, value, &mut rects);
            candidates.extend(
                rects
                    .into_iter()
                    .filter(|rect| rect.top() >= table_top)
                    .map(|rect| (rect, clipped.clip_rect)),
            );
        }
        let fully_visible = candidates
            .iter()
            .any(|(rect, clip)| clip.contains_rect(*rect));
        assert!(
            fully_visible,
            "Output results must show the complete identity/Fit value `{value}`: {candidates:?}"
        );
    }
    harness.snapshot("app_transfer_output");
}

/// A tall canvas so the whole centre column (incl. the results table, which at
/// 800px can fall below the fold) is visible in one image for inspection.
#[test]
fn render_tall_inspection() {
    render("app_tall", egui::Vec2::new(1280.0, 1500.0));
}

#[test]
fn render_tall_empty_inspection() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness_at_size(
        ParamExApp::from_session(Session::new()),
        egui::Vec2::new(1280.0, 1500.0),
    );
    harness.snapshot("app_tall_empty");
}

#[test]
fn render_transfer_output_tall_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        transfer_output_harness(egui::Vec2::new(1280.0, 1500.0), seed_transfer_output_app());
    harness.get_by_label("VG 1 \u{2192} 5 V");
    harness.snapshot("app_transfer_output_tall");
}

#[test]
fn render_transfer_output_tall_empty_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = transfer_output_harness(egui::Vec2::new(1280.0, 1500.0), seed_app());
    harness.snapshot("app_transfer_output_tall_empty");
}

/// A maximized-style 1920×1080 window: proves the responsive shell balances the
/// columns (side columns at their caps) and grows the center graphs/table to fill,
/// rather than ballooning one column or overlapping.
#[test]
fn render_wide_window() {
    render("app_wide", egui::Vec2::new(1920.0, 1080.0));
}

/// Variant buttons must give hover feedback — the old explicit
/// `Button::fill` pinned every state, so nothing changed under the pointer.
/// Two scenes: a hovered filled-primary (darkened fill) and a hovered
/// outlined-danger (red wash). At-rest renders are covered by the unchanged
/// `app_*` baselines (the rework is contractually rest-identical).
#[test]
fn render_button_hover_primary() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());
    let center = harness.get_by_label("Load Transfer").rect().center();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(center));
    harness.run();
    harness.run();
    harness.snapshot("app_button_hover_primary");
}

#[test]
fn render_button_hover_danger() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());
    let center = harness.get_by_label("Clear All").rect().center();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(center));
    harness.run();
    harness.run();
    harness.snapshot("app_button_hover_danger");
}

/// An INACTIVE segment washes on hover (Banner: light
/// wash on the ink track; Card: page tint one step darker). The hover is
/// hand-rolled off the previous frame's response, so it needs its own
/// pointer-driven render guards.
#[test]
fn render_segment_hover_banner() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_app());
    // On the Transfer page the banner's inactive "TLM" segment is the only
    // node with that exact label.
    let center = harness.get_by_label("TLM").rect().center();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(center));
    harness.run();
    harness.run();
    harness.snapshot("app_segment_hover_banner");
}

#[test]
fn render_segment_hover_card_tab() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_tlm_app());
    // The TLM results card's inactive middle tab (markup strips to this text).
    let center = harness.get_by_label("Fits vs VG").rect().center();
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(center));
    harness.run();
    harness.run();
    harness.snapshot("app_segment_hover_card_tab");
}

#[test]
fn render_many_files_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_many_files_app());
    harness.snapshot("app_many_files");
}

#[test]
fn render_selected_warning_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_selected_warning_app());
    let warning_badge = harness
        .query_all_by_label("WARN")
        .map(|node| node.rect())
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("SELECTED header should show a WARN badge");
    assert!(warning_badge.is_positive());
    let header_filename = harness
        .query_all_by_label("partial_curve.csv")
        .map(|node| node.rect())
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("SELECTED header should name the partial result");
    assert!(header_filename.is_positive());
    assert!(harness.get_by_label("Clear All").rect().is_positive());
    harness.snapshot("app_selected_warning");

    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(warning_badge.center()));
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Some metrics could not be extracted.");
}

fn seed_tlm_load_error_app() -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Tlm);
    app.tlm_mut()
        .set_load_error("No valid TLM workbooks were found.".to_string());
    app
}

/// Seed the Model Fit workspace with the synthetic demo devices so the snapshot
/// exercises the real page: the DEVICES list, the FIT plot, the
/// PARAMETERS table, and the SELECTED DEVICE summary.
fn seed_modelfit_app() -> ParamExApp {
    common::loaded_modelfit_app()
}

fn seed_modelfit_real_pair_app(transfer_name: &str, output_name: &str, model: usize) -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    common::modelfit::install_fixture_with_output(app.modelfit_mut(), transfer_name, output_name);
    assert!(app.modelfit_mut().set_selected_model(model));
    app
}

fn seed_modelfit_real_transfer_app(name: &str, model: usize) -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    common::modelfit::install_fixture(app.modelfit_mut(), name);
    assert!(app.modelfit_mut().set_selected_model(model));
    app
}

fn seed_modelfit_manual_app() -> ParamExApp {
    let mut app = seed_modelfit_app();
    let fit = *app
        .modelfit()
        .selected_entry()
        .expect("selected demo device")
        .device()
        .aostft_fit();
    assert!(app
        .modelfit_mut()
        .set_selected_fit(fit.vt + 1.0, fit.gamma, fit.k)
        .is_ok());
    app
}

#[test]
fn render_tlm_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_tlm_app());
    assert!(harness
        .query_by_label("Nearest measured gate voltage.")
        .is_none());
    assert!(harness
        .query_by_label("Default: strongest median current.")
        .is_none());
    assert!(harness
        .query_by_label("Used on next Load Folder.")
        .is_none());
    assert!(harness.query_by_label("Fit OK.").is_none());
    harness.get_by_label("VG -40 V");
    harness.snapshot("app_tlm");
}

#[test]
fn render_tlm_clean_group_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = seed_tlm_app();
    let clean_group = app
        .tlm()
        .group_list()
        .expect("corpus analyzed")
        .groups
        .iter()
        .find(|group| group.warnings.is_empty())
        .map(|group| group.group.clone())
        .expect("corpus contains a clean-fit group");
    assert!(app.tlm_mut().select_group(&clean_group));
    let mut harness = common::app_harness(app);
    harness.get_by_label("Fit quality acceptable.");
    harness.snapshot("app_tlm_clean");
}

/// A p-channel, transfer-only, dual-sweep device — the shape of the user's real
/// uniformity files. Output curves are OPTIONAL, so the SELECTED DEVICE panel
/// shows the Parameters header export action and the detected channel =
/// p-channel; the FIT shows the p-channel transfer (on toward negative
/// Vg), and the OUTPUT panel shows the predicted Id-Vd family from the
/// transfer fit (no measured Id-Vd exists for these files).
fn seed_modelfit_no_output_app() -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    // n-channel base (VT=2, on at high Vg), then mirror to p-channel (Vg -> -Vg)
    // and double back into a hysteresis sweep (5 -> -10 -> 5), like the .xlsx data.
    let nvg: Vec<f64> = (0..=120).map(|i| -3.0 + i as f64 * 0.1).collect();
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let nid = common::modelfit::synthetic_unified_transfer(2.0, 0.5, 1.0e-6, &sub, &nvg);
    let mut vg: Vec<f64> = nvg.iter().map(|v| -v).collect();
    let mut id = nid.clone();
    vg.extend(nvg.iter().rev().map(|v| -v));
    id.extend(nid.iter().rev().copied());
    let device = common::modelfit::fit_device("pch_dual.xlsx", vg, id);
    common::modelfit::install_device(app.modelfit_mut(), device);
    app
}

#[test]
fn render_modelfit_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_app());
    assert!(harness.query_by_label("EXPORT").is_none());
    assert!(harness.query_by_label("Copy Verilog-A").is_none());
    assert!(harness.query_by_label("Full fit ready.").is_none());
    assert!(harness
        .query_by_label("Transfer fit ready. Output defaults.")
        .is_none());
    harness.snapshot("app_modelfit");
}

#[test]
fn render_modelfit_real_pair_aostft_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_real_pair_app("2-6.xlsx", "2-6o.xlsx", 0));
    harness.snapshot("app_modelfit_real_2_6_aostft");
}

#[test]
fn render_modelfit_real_pair_level62_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_real_pair_app("2-6.xlsx", "2-6o.xlsx", 1));
    harness.snapshot("app_modelfit_real_2_6_level62");
}

#[test]
fn render_modelfit_real_3_52_aostft_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness(seed_modelfit_real_pair_app("3-52.xlsx", "3-52o.xlsx", 0));
    harness.snapshot("app_modelfit_real_3_52_aostft");
}

#[test]
fn render_modelfit_real_3_52_level62_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness(seed_modelfit_real_pair_app("3-52.xlsx", "3-52o.xlsx", 1));
    harness.snapshot("app_modelfit_real_3_52_level62");
}

#[test]
fn render_modelfit_real_leakage_transfer_aostft_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_real_transfer_app("1-1.xlsx", 0));
    harness.snapshot("app_modelfit_real_1_1_aostft");
}

#[test]
fn render_modelfit_real_crossover_level62_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_real_transfer_app("7-3.xlsx", 1));
    harness.snapshot("app_modelfit_real_7_3_level62");
}

#[test]
fn render_modelfit_manual_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_manual_app());
    assert!(!harness
        .get_by_label("Reset to Auto")
        .accesskit_node()
        .is_disabled());
    harness.snapshot("app_modelfit_manual");
}

/// A maximized 1920×1080 Model Fit window: the six center tiles grow to fill
/// while the PARAMETERS card fills the right rail (parallels the Transfer
/// app_wide guard).
#[test]
fn render_modelfit_wide_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness_at_size(seed_modelfit_app(), egui::Vec2::new(1920.0, 1080.0));
    harness.snapshot("app_modelfit_wide");
}

/// A tall 1280×1500 Model Fit window: the center tiles grow above PLOT_TILE_MIN_H and the three
/// columns share one bottom seam — pins the tall extreme (parallels the Transfer app_tall guard).
#[test]
fn render_modelfit_tall_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness_at_size(seed_modelfit_app(), egui::Vec2::new(1280.0, 1500.0));
    harness.snapshot("app_modelfit_tall");
}

#[test]
fn modelfit_model_dropdown_defaults_to_aostft() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness =
        common::app_harness_with_runs(seed_modelfit_app(), egui::Vec2::new(1280.0, 800.0), 1);
    // The selector defaults to AOSTFT (index 0); the app renders the dropdown
    // without panicking.
    assert_eq!(harness.state().modelfit().selected_model(), 0);
}

/// The Model Fit page with the stabilized Level 62-derived (LTPS / poly-Si) model selected: the
/// SELECTED DEVICE card, PARAMETERS table, and FIT show the Level 62
/// parameter set in place of AOSTFT while keeping the selected device visible.
fn seed_modelfit_level62_app() -> ParamExApp {
    let mut app = seed_modelfit_app();
    // Index 1 = "Level 62 / LTPS". Built, so the switch commits.
    assert!(app.modelfit_mut().set_selected_model(1));
    app
}

fn seed_modelfit_level62_warning_app() -> ParamExApp {
    let mut app = seed_modelfit_level62_app();
    let state = app.modelfit_mut();
    let mut params = state
        .selected_entry()
        .and_then(|entry| entry.device().level62())
        .map(|fit| fit.params)
        .expect("selected demo device has a Level 62 fit");
    params.vto += 20.0;
    state
        .set_selected_level62_params(params)
        .expect("deliberately poor manual parameters remain valid");
    assert!(
        state
            .selected_entry()
            .and_then(|entry| entry.device().model(FitModel::Level62).r2())
            .is_some_and(|r2| r2 < paramex_gui::format_ui::LOW_R2_THRESHOLD),
        "snapshot setup must retain a reachable low-R² fit"
    );
    app
}

fn seed_modelfit_level62_empty_app() -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    assert!(app.modelfit_mut().set_selected_model(1));
    app
}

#[test]
fn render_modelfit_level62_empty_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_level62_empty_app());
    harness.snapshot("app_modelfit_level62_empty");
}

#[test]
fn render_modelfit_level62_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_level62_app());
    assert!(harness.query_by_label("Fit ready.").is_none());
    harness.snapshot("app_modelfit_level62");
}

#[test]
fn render_modelfit_level62_advanced_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_level62_warning_app());
    harness.get_by_label("Advanced / constants").click();
    harness.run();
    let advanced = harness.get_by_label("Advanced / constants").rect();
    let warning = harness
        .get_by_label(paramex_gui::format_ui::LOW_R2_MESSAGE)
        .rect();
    let vkink = harness.get_by_label("VKINK (V)").rect();
    assert!(
        advanced.bottom() <= warning.top() && warning.bottom() <= vkink.top(),
        "the expanded Level 62 warning must remain visible before its advanced fields: advanced={advanced:?}, warning={warning:?}, VKINK={vkink:?}"
    );
    harness.snapshot("app_modelfit_level62_advanced");
}

#[test]
fn modelfit_selects_level62_and_each_device_has_a_level62_fit() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = common::app_harness_with_runs(
        seed_modelfit_level62_app(),
        egui::Vec2::new(1280.0, 800.0),
        1,
    );
    // The model commits and the page renders it without panicking.
    assert_eq!(harness.state().modelfit().selected_model(), 1);
    // The selector is a display switch: every device carries a Level 62 fit.
    assert!(harness
        .state()
        .modelfit()
        .devices()
        .iter()
        .all(|entry| entry.device().level62().is_some()));
}

/// A P-CHANNEL device rendered with the AOSTFT overlay — the visual guard for the
/// device-frame/n-frame overlay bug (the FIT model line must TRACK the measured
/// p-channel turn-on, not mirror it). A state assertion missed this; a render would not.
fn seed_modelfit_pchannel_app() -> ParamExApp {
    let mut app = seed_modelfit_app();
    let st = app.modelfit_mut();
    st.clear();
    // p-channel device: |Id| rises as Vg goes negative (turn-on near −2 V, device frame).
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let vg_n: Vec<f64> = (0..=200).map(|i| -4.0 + 0.1 * i as f64).collect();
    let id = common::modelfit::synthetic_unified_transfer(2.0, 0.5, 1.0e-6, &sub, &vg_n);
    let vg_p: Vec<f64> = vg_n.iter().map(|&v| -v).collect();
    let device = common::modelfit::fit_device("demo: p-ch", vg_p, id);
    common::modelfit::install_device(st, device);
    // AOSTFT (default model 0); its overlay folds the device-frame gate like every model.
    app
}

#[test]
fn render_modelfit_pchannel_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_pchannel_app());
    for (tick, expected) in [("-5", 4), ("-10", 4), ("-15", 2)] {
        let count: usize = harness
            .output()
            .shapes
            .iter()
            .map(|shape| painted_text_count(&shape.shape, tick))
            .sum();
        assert_eq!(
            count, expected,
            "the p-channel plots must paint every expected {tick} V tick"
        );
    }
    harness.snapshot("app_modelfit_pchannel");
}

/// Regression guard for the short-window (1366×768 laptop) layout: the right
/// rail and center plots stay readable, with PARAMETERS scrolling internally
/// rather than forcing the graph tiles into thin strips.
#[test]
fn render_modelfit_short_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness_at_size(seed_modelfit_app(), egui::Vec2::new(1366.0, 768.0));
    harness.snapshot("app_modelfit_short");
}

#[test]
fn render_modelfit_no_output_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_no_output_app());
    assert!(harness.query_by_label("predicted AOSTFT model").is_none());
    assert!(harness
        .query_by_label("pch_dual.xlsx \u{00B7} predicted")
        .is_none());
    let model_legend_count = harness.get_all_by_label("model").count();
    assert!(
        model_legend_count >= 2,
        "FIT and predicted OUTPUT should both label their visible model curves; saw {model_legend_count}"
    );
    harness.snapshot("app_modelfit_no_output");
}

/// A device whose Id-Vd curves fail to produce output params. The measured
/// family remains visible while both output tiles explain the failed fit.
fn seed_modelfit_nofit_app() -> ParamExApp {
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    let vg: Vec<f64> = (0..=120).map(|i| -2.0 + i as f64 * 0.1).collect();
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let id = common::modelfit::synthetic_unified_transfer(3.0, 0.5, 1.0e-6, &sub, &vg);
    let mut device = common::modelfit::fit_device("B_1.xlsx", vg, id);
    // Below-threshold Id-Vd sub-sweeps have no on-state output fit, but the raw
    // measured family remains available for display.
    let vds: Vec<f64> = (0..=30).map(|i| i as f64 * 0.5).collect();
    let curves: Vec<OutputCurve> = [0.0, 1.0, 2.0]
        .iter()
        .map(|&vg| OutputCurve {
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
    let primary_source = PrimaryTransferSource::new(device.name().to_owned(), None).unwrap();
    assert_eq!(
        app.modelfit_mut()
            .install_fitted_device(
                device,
                primary_source,
                Some(OutputSource::new("B_1_output.xlsx", None).unwrap()),
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    let device = app
        .modelfit()
        .selected_entry()
        .expect("selected device")
        .device();
    assert!(device.has_output_curves());
    assert!(!device.has_output());
    app
}

#[test]
fn render_modelfit_nofit_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_modelfit_nofit_app());
    assert_eq!(
        harness.get_all_by_label("Output fit failed.").count(),
        4,
        "DEVICES, PARAMETERS, OUTPUT FIT, and OUTPUT CONDUCTANCE should all expose the failed output fit"
    );
    assert!(harness
        .query_by_label("B_1.xlsx \u{00B7} no model fit")
        .is_none());

    let gds_title = harness.get_by_label("OUTPUT CONDUCTANCE").rect();
    let next_row_top = harness.get_by_label("INTRINSIC GAIN").rect().top();
    let row_legends: Vec<_> = harness
        .get_all_by_label("measured")
        .map(|node| node.rect())
        .filter(|rect| rect.top() > gds_title.bottom() && rect.bottom() < next_row_top)
        .collect();
    let gm_legend = row_legends
        .iter()
        .copied()
        .find(|rect| rect.center().x < gds_title.left())
        .expect("Transconductance should keep its measured legend");
    let gds_legend = row_legends
        .iter()
        .copied()
        .find(|rect| rect.center().x > gds_title.left())
        .expect("Output Conductance should keep its measured legend");
    assert!(
        common::raster_pixel(gds_legend.bottom())
            <= common::raster_pixel(gm_legend.bottom()),
        "failed Output Conductance legend must stay within its sibling footer: gm={gm_legend:?}, gds={gds_legend:?}"
    );
    harness.snapshot("app_modelfit_nofit");
}

#[test]
fn render_tlm_load_error_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(seed_tlm_load_error_app());
    let _ = harness.get_by_label("Dismiss");
    assert!(harness.query_by_label("Folder layout:").is_none());
    assert!(harness
        .get_by_label("Export Sweep CSV")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .get_by_label("Export TLM CSV")
        .accesskit_node()
        .is_disabled());
    assert!(harness.get_by_label("Clear All").rect().is_positive());
    harness.snapshot("app_tlm_load_error");
}

#[test]
fn tlm_load_error_stays_inside_bento_at_screenshot_height() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness =
        common::app_harness_at_size(seed_tlm_load_error_app(), egui::Vec2::new(1280.0, 759.0));

    assert!(harness.query_by_label("Folder layout:").is_none());
    assert!(harness.get_by_label("Clear All").rect().is_positive());
    let groups = harness.get_by_label("GROUPS").rect();
    let results = harness.get_by_label("RESULTS").rect();
    assert!(
        groups.top() <= results.top() - 20.0,
        "TLM load-error state should spend the compact DATA height on GROUPS: GROUPS top {} vs RESULTS top {}",
        groups.top(),
        results.top()
    );
    harness.snapshot("app_tlm_load_error_759");
}

#[test]
fn render_tlm_sweep_tab() {
    // A load always lands on the Results tab (covered by `app_tlm`), so force the
    // V_G-sweep tab over the same seed — without this scene the sweep table body
    // would have zero render coverage.
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = seed_tlm_app();
    app.tlm_mut()
        .set_results_tab(paramex_gui::workspaces::tlm::state::TlmTab::Sweep);
    let mut harness = common::app_harness(app);
    harness.snapshot("app_tlm_sweep");
}

/// The V_G picker strip with its thumb MID-RAIL: guards that the rail stays a
/// plain grey track with NO trailing fill (a fill-to-thumb reads as range
/// semantics, but this strip picks a single point;
/// every other TLM scene has the thumb at index 0 where a regression would be
/// zero-width and invisible). Also exercises a non-default gate voltage
/// through the analysis/results cards.
#[test]
fn render_tlm_mid_vg() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = seed_tlm_app();
    let mid = {
        let vgs = app.tlm().vg_picker().expect("corpus analyzed").vg_values;
        vgs[vgs.len() / 2]
    };
    app.tlm_mut().recompute_at_vg(mid);
    let mut harness = common::app_harness(app);
    harness.get_by_label("VG -20 V");
    harness.snapshot("app_tlm_mid_vg");
}

#[test]
fn render_tlm_tall_inspection() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness_at_size(seed_tlm_app(), egui::Vec2::new(1280.0, 1500.0));
    harness.snapshot("app_tlm_tall");
}

#[test]
fn tlm_data_stays_content_fit_at_tall_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = common::app_harness_at_size(seed_tlm_app(), egui::Vec2::new(1280.0, 1500.0));

    let data = harness.get_by_label("DATA").rect();
    let analysis = harness.get_by_label("ANALYSIS").rect();
    let groups = harness.get_by_label("GROUPS").rect();

    assert!(
        analysis.top() - data.top() <= 300.0,
        "DATA absorbed tall-window slack: DATA top {} ANALYSIS top {}",
        data.top(),
        analysis.top()
    );
    assert!(
        groups.top() - analysis.top() <= 230.0,
        "tall-window slack should move below the input pair, not inside DATA: \
         ANALYSIS top {} GROUPS top {}",
        analysis.top(),
        groups.top()
    );
}

#[test]
fn render_empty_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(ParamExApp::from_session(Session::new()));
    harness.snapshot("app_empty");
}

#[test]
fn render_modelfit_empty_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Model);
    let mut harness = common::app_harness(app);
    harness.get_by_label("Load Transfer");
    harness.get_by_label("TRANSFER FIT");
    assert!(harness
        .query_by_label("Select a device to see its transfer curve and the AOSTFT model fit.")
        .is_none());
    assert!(harness
        .query_by_label("Select a device to see its output fit.")
        .is_none());
    assert!(harness
        .query_by_label("Select a device to see its transconductance.")
        .is_none());
    assert!(harness
        .query_by_label("Select a device to see its full parameter set.")
        .is_none());
    assert!(harness
        .query_by_label("Load files to see fitted devices.")
        .is_none());
    assert!(harness.query_by_label("EXPORT").is_none());
    harness.snapshot("app_modelfit_empty");
}

#[test]
fn empty_transfer_window_keeps_primary_actions_visible() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = common::app_harness(ParamExApp::from_session(Session::new()));

    assert!(harness.get_by_label("Load Transfer").rect().is_positive());
    assert!(harness.get_by_label("Load Output").rect().is_positive());
    assert!(harness.get_by_label("Load Folder").rect().is_positive());
    assert!(harness.get_by_label("FIT").rect().is_positive());
    assert!(harness.get_by_label("SELECTED").rect().is_positive());
    assert!(harness
        .query_by_label("Load or select a transfer curve to see transfer fit.")
        .is_none());
    assert!(harness
        .query_by_label("Load or select a transfer curve to see file metrics.")
        .is_none());
    assert!(harness.get_by_label("RESULTS").rect().is_positive());
    assert!(harness
        .query_by_label("No transfer results to show.")
        .is_none());
    assert!(harness
        .get_by_label("Export CSV")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .get_by_label("Transfer Fit")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .get_by_label("Output Fit")
        .accesskit_node()
        .is_disabled());
    assert!(harness.get_by_label("GEOMETRY").rect().is_positive());
    assert!(harness
        .get_by_label("Apply W/L to All Files")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .get_by_label("Measured Cox (nF/cm2)")
        .rect()
        .is_positive());
    assert!(harness.query_by_label("Remove layer").is_none());
    assert!(harness.get_by_label("Estimate Cox").rect().is_positive());
    assert!(harness.query_by_label("Use Estimated Cox").is_none());
}

/// Clicking the banner TLM segment switches the workspace; the empty TLM page
/// keeps the primary load action and card shell visible.
#[test]
fn tlm_toggle_switches_and_empty_states_render() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut harness = common::app_harness(ParamExApp::from_session(Session::new()));
    harness.get_by_label("TLM").click();
    harness.run();
    let _ = harness.get_by_label("DATA");
    let _ = harness.get_by_label("GROUPS");
    let _ = harness.get_by_label("FIT");
    let _ = harness.get_by_label("RESULTS");
    let _ = harness.get_by_label("SELECTED");
    assert!(harness.query_by_label("Folder layout:").is_none());
    let _ = harness.get_by_label("Load Folder");
}

/// Empty-TLM snapshot: primary actions stay visible while cold-start cards stay quiet.
#[test]
fn render_tlm_empty_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut app = ParamExApp::from_session(Session::new());
    app.set_active_workspace(Workspace::Tlm);
    let mut harness = common::app_harness(app);
    assert!(harness
        .get_by_label("Export Sweep CSV")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .get_by_label("Export TLM CSV")
        .accesskit_node()
        .is_disabled());
    assert!(harness
        .query_by_label("Load TLM workbooks to see TLM results.")
        .is_none());
    assert!(harness
        .query_by_label("Load TLM workbooks to see the TLM fit.")
        .is_none());
    harness.snapshot("app_tlm_empty");
}

#[test]
fn tlm_input_cards_stay_content_fit_at_short_window() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = common::app_harness_at_size(seed_tlm_app(), egui::Vec2::new(1280.0, 720.0));

    let groups = harness.get_by_label("GROUPS").rect();
    let results = harness.get_by_label("RESULTS").rect();
    assert!(
        groups.top() <= results.top() - 20.0,
        "TLM input cards should stay content-fit at a short window: \
         GROUPS top {} vs RESULTS top {}",
        groups.top(),
        results.top()
    );
}
