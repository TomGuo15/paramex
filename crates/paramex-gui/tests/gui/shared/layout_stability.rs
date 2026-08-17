use std::sync::Mutex;

use egui_kittest::{
    kittest::{AccessKitNode, NodeT, Queryable},
    Harness,
};
use paramex_core::transfer::{parse_transfer_file, OutputCurve, OutputDataset, Session};
use paramex_gui::{app::ParamExApp, state::Workspace};

static HARNESS_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn technical_guide_navigation_stays_fixed_while_model_body_scrolls() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for pixels_per_point in crate::common::RASTER_TEST_SCALES {
        let mut harness = app_harness_at_scale(
            egui::vec2(1280.0, 800.0),
            pixels_per_point,
            crate::common::empty_workspace_app(Workspace::Model),
        );

        harness.get_by_label("Data guide").click_accesskit();
        harness.run();
        harness.run();

        let title = harness
            .query_by_label("TECHNICAL GUIDE")
            .unwrap_or_else(|| panic!("guide should open at {pixels_per_point}x"))
            .rect();
        let workspace_tab = harness.get_by_label("Model Fit guide").rect();
        let model_tab = harness.get_by_label("Level 62 / LTPS").rect();

        harness
            .get_by_label("Analog terminal charge")
            .scroll_to_me();
        harness.run();
        harness.run();

        for (label, before) in [
            ("TECHNICAL GUIDE", title),
            ("Model Fit guide", workspace_tab),
            ("Level 62 / LTPS", model_tab),
        ] {
            crate::common::assert_same_raster_rect(
                &format!("guide fixed navigation {label} at {pixels_per_point}x"),
                before,
                harness.get_by_label(label).rect(),
                pixels_per_point,
            );
        }
    }
}

#[test]
fn technical_guide_shell_stays_fixed_across_pages() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for pixels_per_point in crate::common::RASTER_TEST_SCALES {
        let mut harness = app_harness_at_scale(
            egui::vec2(1280.0, 800.0),
            pixels_per_point,
            crate::common::empty_workspace_app(Workspace::Transfer),
        );

        harness.get_by_label("Data guide").click_accesskit();
        harness.run();
        harness.run();

        let title = harness.get_by_label("TECHNICAL GUIDE").rect();
        let close = harness.get_by_label("Close guide").rect();

        for page in ["TLM guide", "Model Fit guide", "Level 62 / LTPS"] {
            harness.get_by_label(page).click_accesskit();
            harness.run();
            harness.run();

            crate::common::assert_same_raster_rect(
                &format!("guide shell title on {page} at {pixels_per_point}x"),
                title,
                harness.get_by_label("TECHNICAL GUIDE").rect(),
                pixels_per_point,
            );
            let current_close = harness.get_by_label("Close guide").rect();
            for (edge, before, current) in [
                ("left", close.left(), current_close.left()),
                ("right", close.right(), current_close.right()),
            ] {
                crate::common::assert_same_raster_edge(
                    &format!("guide shell close {edge} on {page} at {pixels_per_point}x"),
                    before,
                    current,
                    pixels_per_point,
                );
            }
        }
    }
}

#[test]
fn technical_guide_contract_keys_align_with_description_tops() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for pixels_per_point in crate::common::RASTER_TEST_SCALES {
        let mut harness = app_harness_at_scale(
            egui::vec2(1280.0, 800.0),
            pixels_per_point,
            crate::common::empty_workspace_app(Workspace::Model),
        );

        harness.get_by_label("Data guide").click_accesskit();
        harness.run();
        harness.run();
        harness.get_by_label("Level 62 / LTPS").click_accesskit();
        harness.run();
        harness.run();

        for (key_label, description_label) in [
            ("Files", ".csv · .tsv · .txt · .xlsx · .xls"),
            ("Transfer", "VG · ID. Set its VDS in Parameters."),
            ("Output", "VG · VD · ID."),
            ("DIBL", "Second transfer with VG · VD · ID."),
            ("C-V", "Bias · capacitance. Updates Cox."),
        ] {
            let key = harness
                .get_all_by_label(key_label)
                .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
                .unwrap_or_else(|| panic!("Model Fit guide should expose {key_label:?}"));
            let description = harness.get_by_label(description_label);
            let key_line = key
                .children()
                .find(|node| node.accesskit_node().role() == egui::accesskit::Role::TextRun)
                .unwrap_or_else(|| panic!("{key_label:?} should expose its painted text line"));
            let description_line = description
                .children()
                .find(|node| node.accesskit_node().role() == egui::accesskit::Role::TextRun)
                .unwrap_or_else(|| {
                    panic!("{description_label:?} should expose its painted first text line")
                });

            crate::common::assert_same_raster_edge(
                &format!(
                    "Model Fit {key_label} contract key/description top at {pixels_per_point}x"
                ),
                key_line.rect().top(),
                description_line.rect().top(),
                pixels_per_point,
            );
        }

        for (page, keys) in [
            ("Transfer guide", &["Files", "Columns", "Points"][..]),
            ("TLM guide", &["Files", "Length", "Data", "Bias"][..]),
        ] {
            harness.get_by_label(page).click_accesskit();
            harness.run();
            harness.run();

            let height = harness.get_by_label(keys[0]).rect().height();
            for key in &keys[1..] {
                crate::common::assert_same_raster_edge(
                    &format!("{page} {key} stays on one line at {pixels_per_point}x"),
                    height,
                    harness.get_by_label(key).rect().height(),
                    pixels_per_point,
                );
            }
        }
    }
}

#[test]
fn technical_guide_transfer_and_tlm_fit_without_scrolling() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for pixels_per_point in crate::common::RASTER_TEST_SCALES {
        let mut harness = app_harness_at_scale(
            egui::vec2(1280.0, 800.0),
            pixels_per_point,
            crate::common::empty_workspace_app(Workspace::Transfer),
        );

        harness.get_by_label("Data guide").click_accesskit();
        harness.run();
        harness.run();

        for (tab, first_label, final_label) in [
            (
                None,
                "INPUT",
                "Hysteresis needs forward + reverse sweeps (≥12 points each).",
            ),
            (
                Some("TLM guide"),
                "INPUT",
                "Need ≥2 lengths (≥3 for R2). Primary: highest-current device per L; median: diagnostic. m is slope (Ω/μm), not sheet resistance.",
            ),
        ] {
            if let Some(tab) = tab {
                harness.get_by_label(tab).click_accesskit();
                harness.run();
                harness.run();
            }

            let first = harness.get_by_label(first_label).rect();
            let last = harness.get_by_label(final_label).rect();
            harness.get_by_label(final_label).scroll_to_me();
            harness.run();
            harness.run();

            crate::common::assert_same_raster_rect(
                &format!("{first_label} remains visible at {pixels_per_point}x"),
                first,
                harness.get_by_label(first_label).rect(),
                pixels_per_point,
            );
            crate::common::assert_same_raster_rect(
                &format!("{final_label} needs no scroll at {pixels_per_point}x"),
                last,
                harness.get_by_label(final_label).rect(),
                pixels_per_point,
            );
        }
    }
}

#[test]
fn transfer_structural_rects_stay_fixed_between_empty_and_loaded() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Transfer));
    let loaded = crate::common::app_harness(crate::common::loaded_transfer_app());

    assert_rects_stable("Transfer", &empty, &loaded, TRANSFER_STABLE_LABELS);
    assert_no_widget_overflows_cards("Transfer empty", &empty, &TRANSFER_CARDS);
    assert_no_widget_overflows_cards("Transfer loaded", &loaded, &TRANSFER_CARDS);
}

#[test]
fn transfer_structural_rects_stay_fixed_between_empty_and_error() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Transfer));
    let mut error_app = crate::common::empty_workspace_app(Workspace::Transfer);
    error_app.transfer_mut().record_ingest_error(
        "bad_transfer.csv".to_string(),
        "No usable transfer curve".to_string(),
    );
    let error = crate::common::app_harness(error_app);

    assert_rects_stable("Transfer error", &empty, &error, TRANSFER_STABLE_LABELS);
    assert_no_widget_overflows_cards("Transfer error", &error, &TRANSFER_CARDS);
}

#[test]
fn transfer_terminal_controls_keep_tight_card_bottom_margins() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let shell = paramex_gui::layout::ShellRects::from_content(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1280.0, 800.0),
    ));
    let top_h = paramex_gui::layout::fixed_bottom_stack(
        shell.center.height(),
        paramex_gui::layout::SELECTED_METRICS_HEIGHT,
    )
    .top;
    let center_top =
        egui::Rect::from_min_size(shell.center.min, egui::vec2(shell.center.width(), top_h));

    for (scene, harness) in [
        (
            "FIT empty",
            crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Transfer)),
        ),
        (
            "FIT loaded",
            crate::common::app_harness(crate::common::loaded_transfer_app()),
        ),
    ] {
        assert_tight_bottom_in_rect(scene, &harness, center_top, "VG max");
    }

    for (scene, harness) in [
        (
            "OUTPUT empty",
            output_harness_at(
                egui::vec2(1280.0, 800.0),
                crate::common::loaded_transfer_app(),
            ),
        ),
        (
            "OUTPUT loaded",
            output_harness_at(
                egui::vec2(1280.0, 800.0),
                crate::common::loaded_transfer_output_app(),
            ),
        ),
    ] {
        assert_tight_bottom_in_rect(scene, &harness, center_top, "VD max");
    }
}

#[test]
fn transfer_and_output_range_controls_share_terminal_baseline() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    for ppp in crate::common::RASTER_TEST_SCALES {
        assert_transfer_output_range_baseline(egui::vec2(1280.0, 800.0), ppp);
    }
    assert_transfer_output_range_baseline(egui::vec2(1280.0, 1500.0), 1.0);
}

fn assert_transfer_output_range_baseline(size: egui::Vec2, pixels_per_point: f32) {
    let transfer =
        app_harness_at_scale(size, pixels_per_point, crate::common::loaded_transfer_app());
    let output = output_harness_at_scale(
        size,
        pixels_per_point,
        crate::common::loaded_transfer_output_app(),
    );
    let vg_max = transfer
        .get_all_by_label("VG max")
        .map(|node| node.rect())
        .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .expect("Transfer range field");
    let vd_max = output.get_by_label("VD max").rect();

    crate::common::assert_same_raster_edge(
        &format!("Transfer and Output range-row top at {size:?}, {pixels_per_point} ppp"),
        vg_max.top(),
        vd_max.top(),
        pixels_per_point,
    );
    crate::common::assert_same_raster_edge(
        &format!("Transfer and Output range-row bottom at {size:?}, {pixels_per_point} ppp"),
        vg_max.bottom(),
        vd_max.bottom(),
        pixels_per_point,
    );
}

#[test]
fn transfer_output_warning_stays_inside_the_loaded_center_card() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let size = egui::vec2(1280.0, 759.0);
    let harness = output_harness_at(size, loaded_transfer_output_warning_app());
    let warning = harness.get_by_label("No finite Id-Vd points").rect();
    let shell = paramex_gui::layout::ShellRects::from_content(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        size,
    ));
    let top_h = paramex_gui::layout::fixed_bottom_stack(
        shell.center.height(),
        paramex_gui::layout::SELECTED_METRICS_HEIGHT,
    )
    .top;
    let output_card =
        egui::Rect::from_min_size(shell.center.min, egui::vec2(shell.center.width(), top_h));
    let results_top = output_card.bottom() + paramex_gui::layout::CARD_GAP;

    assert!(
        warning.bottom() <= output_card.bottom() && warning.bottom() < results_top,
        "loaded Output warning overlaps the next center-column card: warning={warning:?}, output={output_card:?}, results_top={results_top:.1}"
    );
}

#[test]
fn tlm_structural_rects_stay_fixed_between_empty_and_loaded() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Tlm));
    let loaded = crate::common::app_harness(crate::common::loaded_tlm_app());

    assert_rects_stable("TLM", &empty, &loaded, TLM_STABLE_LABELS);
    assert_no_widget_overflows_cards("TLM empty", &empty, &TLM_CARDS);
    assert_no_widget_overflows_cards("TLM loaded", &loaded, &TLM_CARDS);
}

#[test]
fn tlm_structural_rects_stay_fixed_between_empty_and_error() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Tlm));
    let mut error_app = crate::common::empty_workspace_app(Workspace::Tlm);
    error_app
        .tlm_mut()
        .set_load_error("No valid TLM workbooks were found.".to_string());
    let error = crate::common::app_harness(error_app);

    assert_rects_stable("TLM error", &empty, &error, TLM_STABLE_LABELS);
    assert_no_widget_overflows_cards("TLM error", &error, &TLM_CARDS);
}

#[test]
fn modelfit_structural_rects_stay_fixed_between_empty_and_loaded() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Model));
    let loaded = crate::common::app_harness(crate::common::loaded_modelfit_app());

    assert_rects_stable("Model Fit", &empty, &loaded, MODELFIT_STABLE_LABELS);
    assert_no_widget_overflows_cards("Model Fit empty", &empty, &MODELFIT_CARDS);
    assert_no_widget_overflows_cards("Model Fit loaded", &loaded, &MODELFIT_CARDS);
}

#[test]
fn modelfit_parameters_keep_the_terminal_input_at_the_card_bottom() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let card = paramex_gui::layout::ShellRects::from_content(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1280.0, 800.0),
    ))
    .right;
    for (scene, app) in [
        (
            "Model Fit empty",
            crate::common::empty_workspace_app(Workspace::Model),
        ),
        ("Model Fit loaded", crate::common::loaded_modelfit_app()),
    ] {
        let harness = crate::common::app_harness(app);
        let terminal = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .map(|node| node.rect())
            .filter(|rect| card.contains(rect.center()))
            .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
            .expect("PARAMETERS terminal input");
        let inner_bottom = card.bottom() - paramex_gui::ui_kit::CARD_INNER_MARGIN as f32;
        let bottom_slack = crate::common::raster_pixel(inner_bottom)
            - crate::common::raster_pixel(terminal.bottom());
        assert!(
            (0..=6).contains(&bottom_slack),
            "{scene} leaves {bottom_slack}px below the final PARAMETERS input; expected 0-6"
        );
    }
}

#[test]
fn workspace_structural_rects_stay_fixed_at_fractional_dpi() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let size = egui::vec2(1280.0, 800.0);

    for ppp in crate::common::RASTER_TEST_SCALES.into_iter().skip(1) {
        for (workspace, empty, loaded, labels) in [
            (
                "Transfer",
                app_harness_at_scale(
                    size,
                    ppp,
                    crate::common::empty_workspace_app(Workspace::Transfer),
                ),
                app_harness_at_scale(size, ppp, crate::common::loaded_transfer_app()),
                TRANSFER_STABLE_LABELS,
            ),
            (
                "TLM",
                app_harness_at_scale(
                    size,
                    ppp,
                    crate::common::empty_workspace_app(Workspace::Tlm),
                ),
                app_harness_at_scale(size, ppp, crate::common::loaded_tlm_app()),
                TLM_STABLE_LABELS,
            ),
            (
                "Model Fit",
                app_harness_at_scale(
                    size,
                    ppp,
                    crate::common::empty_workspace_app(Workspace::Model),
                ),
                app_harness_at_scale(size, ppp, crate::common::loaded_modelfit_app()),
                MODELFIT_STABLE_LABELS,
            ),
        ] {
            assert_rects_stable(
                &format!("{workspace} at {}%", (ppp * 100.0) as u32),
                &empty,
                &loaded,
                labels,
            );
        }
    }
}

#[test]
fn modelfit_structural_rects_stay_fixed_between_empty_and_error() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Model));
    let mut error_app = crate::common::empty_workspace_app(Workspace::Model);
    error_app.modelfit_workspace_mut().record_ingest_error(
        "bad_model.csv".to_string(),
        "No usable transfer curve".to_string(),
    );
    let error = crate::common::app_harness(error_app);

    assert_rects_stable("Model Fit error", &empty, &error, MODELFIT_STABLE_LABELS);
    assert_no_widget_overflows_cards("Model Fit error", &error, &MODELFIT_CARDS);
}

#[test]
fn workspace_pages_share_the_same_reference_column_widths() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let transfer =
        crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Transfer));
    let tlm = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Tlm));
    let model = crate::common::app_harness(crate::common::empty_workspace_app(Workspace::Model));

    let transfer_right = transfer.get_by_label("GEOMETRY").rect();
    let tlm_right = tlm.get_by_label("FILES").rect();
    let model_right = model.get_by_label("PARAMETERS").rect();

    assert_x_close(
        "Transfer right column vs TLM right column",
        transfer_right.left(),
        tlm_right.left(),
    );
    assert_x_close(
        "Transfer right column vs Model Fit right column",
        transfer_right.left(),
        model_right.left(),
    );
}

#[test]
fn workspace_columns_share_shell_top_rows() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    for (size_name, size) in [
        ("framed 1280x759", egui::Vec2::new(1280.0, 759.0)),
        ("reference 1280x800", egui::Vec2::new(1280.0, 800.0)),
        ("wide 1600x900", egui::Vec2::new(1600.0, 900.0)),
    ] {
        for (workspace, app, top_labels) in [
            (
                "Transfer",
                crate::common::empty_workspace_app(Workspace::Transfer),
                ["DATA", "FIT", "GEOMETRY"],
            ),
            (
                "TLM",
                crate::common::empty_workspace_app(Workspace::Tlm),
                ["DATA", "FIT", "FILES"],
            ),
            (
                "Model Fit",
                crate::common::empty_workspace_app(Workspace::Model),
                ["DATA", "TRANSFER FIT", "PARAMETERS"],
            ),
        ] {
            let harness = app_harness_at(size, app);
            assert_title_tops_align(workspace, size_name, &harness, &top_labels);
        }
    }
}

#[test]
fn tlm_results_and_selected_share_the_bottom_band() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tlm = crate::common::app_harness(crate::common::loaded_tlm_app());

    let results = tlm.get_by_label("RESULTS").rect();
    let selected = tlm.get_by_label("SELECTED").rect();
    crate::common::assert_same_raster_edge(
        "TLM RESULTS and SELECTED bottom-band seam",
        selected.top(),
        results.top(),
        tlm.ctx.pixels_per_point(),
    );
}

#[test]
fn tlm_content_cards_keep_tight_heights() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tlm = crate::common::app_harness(crate::common::loaded_tlm_app());

    let analysis = tlm.get_by_label("ANALYSIS").rect();
    let groups = tlm.get_by_label("GROUPS").rect();
    assert!(
        groups.top() - analysis.top() <= 160.0,
        "TLM ANALYSIS card should stay tight to its controls: analysis={analysis:?}, groups={groups:?}"
    );
    let shell = paramex_gui::layout::ShellRects::from_content(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1280.0, 800.0),
    ));
    let data_h = paramex_gui::workspaces::tlm::layout::TLM_DATA_CARD_HEIGHT;
    let analysis_h = paramex_gui::workspaces::tlm::layout::TLM_ANALYSIS_HEIGHT;
    let data_card =
        egui::Rect::from_min_size(shell.left.min, egui::vec2(shell.left.width(), data_h));
    let analysis_card = egui::Rect::from_min_size(
        egui::pos2(
            shell.left.left(),
            shell.left.top() + data_h + paramex_gui::layout::CARD_GAP,
        ),
        egui::vec2(shell.left.width(), analysis_h),
    );
    assert_tight_bottom_in_rect("TLM DATA", &tlm, data_card, "Clear All");
    assert_tight_bottom_in_rect("TLM ANALYSIS", &tlm, analysis_card, "VG (V)");
}

#[test]
fn modelfit_graph_tiles_use_the_shell_card_gap() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let harness = crate::common::app_harness(crate::common::loaded_modelfit_app());
    let shell = paramex_gui::layout::ShellRects::from_content(egui::Rect::from_min_max(
        egui::pos2(0.0, 0.0),
        egui::pos2(1280.0, 800.0),
    ));
    let expected_title_delta = (shell.center.width() - paramex_gui::layout::CARD_GAP) * 0.5
        + paramex_gui::layout::CARD_GAP;

    for (left_label, right_label) in [
        ("TRANSFER FIT", "OUTPUT FIT"),
        ("TRANSCONDUCTANCE", "OUTPUT CONDUCTANCE"),
        ("GM/ID SIZING", "INTRINSIC GAIN"),
    ] {
        let left = harness.get_by_label(left_label).rect();
        let right = harness.get_by_label(right_label).rect();
        crate::common::assert_same_raster_edge(
            &format!("Model Fit graph-card gap for {left_label}/{right_label}"),
            right.left(),
            left.left() + expected_title_delta,
            harness.ctx.pixels_per_point(),
        );
    }
}

#[test]
fn workspace_cards_do_not_overflow_at_short_and_reference_sizes() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (size_name, size) in [
        ("framed 1280x759", egui::Vec2::new(1280.0, 759.0)),
        ("laptop 1366x768", egui::Vec2::new(1366.0, 768.0)),
        ("reference 1280x800", egui::Vec2::new(1280.0, 800.0)),
    ] {
        for (scene, app, cards) in [
            (
                "Transfer empty",
                crate::common::empty_workspace_app(Workspace::Transfer),
                TRANSFER_CARDS.as_slice(),
            ),
            (
                "Transfer loaded",
                crate::common::loaded_transfer_app(),
                TRANSFER_CARDS.as_slice(),
            ),
            (
                "TLM empty",
                crate::common::empty_workspace_app(Workspace::Tlm),
                TLM_CARDS.as_slice(),
            ),
            (
                "TLM loaded",
                crate::common::loaded_tlm_app(),
                TLM_CARDS.as_slice(),
            ),
            (
                "Model Fit empty",
                crate::common::empty_workspace_app(Workspace::Model),
                MODELFIT_CARDS.as_slice(),
            ),
            (
                "Model Fit loaded",
                crate::common::loaded_modelfit_app(),
                MODELFIT_CARDS.as_slice(),
            ),
        ] {
            let harness = app_harness_at(size, app);
            assert_no_widget_overflows_cards(&format!("{scene} at {size_name}"), &harness, cards);
        }

        let output = output_harness_at(size, crate::common::loaded_transfer_output_app());
        assert_no_widget_overflows_cards(
            &format!("Transfer Output loaded at {size_name}"),
            &output,
            &TRANSFER_OUTPUT_CARDS,
        );
    }
}

#[test]
fn transfer_tall_plot_groups_stack_without_state_dependent_chrome() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let size = egui::Vec2::new(1280.0, 1500.0);
    let empty = app_harness_at(
        size,
        crate::common::empty_workspace_app(Workspace::Transfer),
    );
    let loaded = app_harness_at(size, crate::common::loaded_transfer_app());

    assert_rects_stable(
        "Transfer tall",
        &empty,
        &loaded,
        &["FIT", "VTH fit range", "SS fit range", "VG min", "VG max"],
    );
    for (scene, harness) in [("empty", &empty), ("loaded", &loaded)] {
        let vt = harness.get_by_label("VTH fit range").rect();
        let ss = harness.get_by_label("SS fit range").rect();
        assert_x_close(
            &format!("Transfer tall {scene} plot titles"),
            ss.left(),
            vt.left(),
        );

        let range_max = rects_by_label(harness, "VG max");
        assert_eq!(range_max.len(), 2, "expected one V_G range per plot group");
        assert!(
            ss.top() > range_max[0].bottom() + 4.0,
            "Transfer tall {scene} SS group should start below the VTH controls: ss={ss:?}, first_max={:?}",
            range_max[0]
        );
        assert_no_widget_overflows_cards(
            &format!("Transfer tall {scene}"),
            harness,
            &TRANSFER_CARDS,
        );
    }
}

#[test]
fn transfer_tall_output_plot_groups_stay_fixed_between_empty_and_loaded() {
    let _guard = HARNESS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let size = egui::Vec2::new(1280.0, 1500.0);
    let empty = output_harness_at(size, crate::common::loaded_transfer_app());
    let loaded = output_harness_at(size, crate::common::loaded_transfer_output_app());

    assert_rects_stable(
        "Transfer tall Output",
        &empty,
        &loaded,
        &["Reset to Auto", "Transfer", "Output", "VD min", "VD max"],
    );
    for (scene, harness) in [("empty", &empty), ("loaded", &loaded)] {
        let transfer = rects_by_label(harness, "Transfer")
            .into_iter()
            .max_by(|a, b| a.top().total_cmp(&b.top()))
            .expect("Transfer subplot caption");
        let output = harness.get_by_label("Output").rect();
        let vd_min = harness.get_by_label("VD min").rect();
        crate::common::assert_same_raster_edge(
            &format!("Transfer tall Output {scene} shared left title rail"),
            transfer.left(),
            output.left(),
            harness.ctx.pixels_per_point(),
        );
        assert!(
            output.top() > transfer.bottom(),
            "Transfer tall Output {scene} plots should stack vertically: transfer={transfer:?}, output={output:?}"
        );
        assert!(
            vd_min.top() > output.bottom(),
            "Transfer tall Output {scene} controls should follow both plots: output={output:?}, vd_min={vd_min:?}"
        );
        assert_no_widget_overflows_cards(
            &format!("Transfer tall Output {scene}"),
            harness,
            &TRANSFER_OUTPUT_CARDS,
        );
    }
}

const TRANSFER_CARDS: [&str; 6] = ["DATA", "FIT", "RESULTS", "SELECTED", "GEOMETRY", "COX"];
const TRANSFER_OUTPUT_CARDS: [&str; 6] = [
    "DATA",
    "Reset to Auto",
    "RESULTS",
    "SELECTED",
    "GEOMETRY",
    "COX",
];
const TRANSFER_STABLE_LABELS: &[&str] = &[
    "DATA",
    "Load Transfer",
    "Load Output",
    "Load Folder",
    "Remove Selected",
    "Clear All",
    "Keep Checked",
    "FIT",
    "VG min",
    "VG max",
    "RESULTS",
    "SELECTED",
    "Transfer Fit",
    "Output Fit",
    "Export CSV",
    "GEOMETRY",
    "Apply W/L to All Files",
    "COX",
    "Measured Cox (nF/cm2)",
    "Estimate Cox",
];

const TLM_CARDS: [&str; 7] = [
    "DATA", "ANALYSIS", "GROUPS", "FIT", "RESULTS", "FILES", "SELECTED",
];
const TLM_STABLE_LABELS: &[&str] = &[
    "DATA",
    "Fallback VD (V)",
    "Load Folder",
    "Clear All",
    "ANALYSIS",
    "Gate voltage VG",
    "VG (V)",
    "GROUPS",
    "FIT",
    "RESULTS",
    "Group fits",
    "Fits vs VG",
    "Rtotal points",
    "Export Sweep CSV",
    "Export TLM CSV",
    "FILES",
    "SELECTED",
];

const MODELFIT_CARDS: [&str; 9] = [
    "DATA",
    "DEVICES",
    "TRANSFER FIT",
    "OUTPUT FIT",
    "TRANSCONDUCTANCE",
    "OUTPUT CONDUCTANCE",
    "GM/ID SIZING",
    "INTRINSIC GAIN",
    "PARAMETERS",
];
const MODELFIT_STABLE_LABELS: &[&str] = &[
    "DATA",
    "Load Transfer",
    "Load Output",
    "Load DIBL",
    "Load C-V",
    "DEVICES",
    "Remove Selected",
    "Clear All",
    "Keep Checked",
    "TRANSFER FIT",
    "OUTPUT FIT",
    "TRANSCONDUCTANCE",
    "OUTPUT CONDUCTANCE",
    "GM/ID SIZING",
    "INTRINSIC GAIN",
    "PARAMETERS",
    "Export Verilog-A",
    "Reset to Auto",
    "channel",
    "transfer R2",
    "Model parameters",
    "Device setup",
];

fn assert_rects_stable(
    workspace: &str,
    empty: &Harness<'_, ParamExApp>,
    loaded: &Harness<'_, ParamExApp>,
    labels: &[&str],
) {
    assert_eq!(
        empty.ctx.pixels_per_point(),
        loaded.ctx.pixels_per_point(),
        "{workspace} state harnesses must use the same display scale"
    );
    for label in labels {
        let empty_rects = rects_by_label(empty, label);
        let loaded_rects = rects_by_label(loaded, label);
        assert!(
            !empty_rects.is_empty(),
            "{workspace} empty state is missing structural node {label:?}"
        );
        assert!(
            !loaded_rects.is_empty(),
            "{workspace} loaded state is missing structural node {label:?}"
        );
        assert_eq!(
            empty_rects.len(),
            loaded_rects.len(),
            "{workspace} structural node count changes for {label:?}: empty={} loaded={}",
            empty_rects.len(),
            loaded_rects.len()
        );
        for (idx, (empty_rect, loaded_rect)) in empty_rects.iter().zip(&loaded_rects).enumerate() {
            assert_same_raster_rect(
                &format!("{workspace} {label:?}[{idx}]"),
                *empty_rect,
                *loaded_rect,
                empty.ctx.pixels_per_point(),
            );
        }
    }
}

fn rects_by_label(harness: &Harness<'_, ParamExApp>, label: &str) -> Vec<egui::Rect> {
    harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .collect()
}

fn app_harness_at(size: egui::Vec2, app: ParamExApp) -> Harness<'static, ParamExApp> {
    app_harness_at_scale(size, 1.0, app)
}

fn app_harness_at_scale(
    size: egui::Vec2,
    pixels_per_point: f32,
    app: ParamExApp,
) -> Harness<'static, ParamExApp> {
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(pixels_per_point)
        .build_eframe(|cc| {
            paramex_gui::theme::install(&cc.egui_ctx);
            app
        });
    harness.run();
    harness.run();
    harness
}

fn output_harness_at(size: egui::Vec2, app: ParamExApp) -> Harness<'static, ParamExApp> {
    output_harness_at_scale(size, 1.0, app)
}

fn output_harness_at_scale(
    size: egui::Vec2,
    pixels_per_point: f32,
    app: ParamExApp,
) -> Harness<'static, ParamExApp> {
    let mut harness = app_harness_at_scale(size, pixels_per_point, app);
    harness.get_by_label("Output Fit").click_accesskit();
    harness.run();
    harness.run();
    harness
}

fn loaded_transfer_output_warning_app() -> ParamExApp {
    let transfer_path = crate::common::crate_file(
        "../paramex-core/tests/reference/parse/fixtures/corpus_double.csv",
    );
    let mut session = Session::new();
    let id = session
        .add_curve(parse_transfer_file(&transfer_path).expect("transfer fixture parses"))
        .expect("transfer fixture adds");
    assert!(session.select_file(&id));
    assert!(session
        .replace_output(
            &id,
            OutputDataset {
                name: "corpus_double_output.csv".to_string(),
                curves: vec![OutputCurve {
                    vg: 5.0,
                    vd: vec![0.0, 1.0, 2.0],
                    id: vec![f64::NAN; 3],
                }],
                source_path: None,
            },
        )
        .is_ok());

    ParamExApp::from_session(session)
}

fn assert_same_raster_rect(
    label: &str,
    empty: egui::Rect,
    loaded: egui::Rect,
    pixels_per_point: f32,
) {
    crate::common::assert_same_raster_rect(label, empty, loaded, pixels_per_point);
}

fn assert_x_close(label: &str, got: f32, expected: f32) {
    crate::common::assert_same_raster_edge(label, got, expected, 1.0);
}

fn assert_title_tops_align(
    workspace: &str,
    size_name: &str,
    harness: &Harness<'_, ParamExApp>,
    labels: &[&str],
) {
    let expected = harness.get_by_label(labels[0]).rect();
    for label in labels.iter().skip(1) {
        let rect = harness.get_by_label(label).rect();
        crate::common::assert_same_raster_edge(
            &format!(
                "{workspace} {label:?} top at {size_name} aligned with {:?}",
                labels[0]
            ),
            rect.top(),
            expected.top(),
            harness.ctx.pixels_per_point(),
        );
    }
}

fn assert_no_widget_overflows_cards(
    scene: &str,
    harness: &Harness<'_, ParamExApp>,
    card_labels: &[&str],
) {
    let cards: Vec<_> = card_labels
        .iter()
        .filter_map(|label| {
            card_node(harness, label).map(|(node, rect)| (*label, node.accesskit_node().id(), rect))
        })
        .collect();

    for node in harness.root().children_recursive() {
        let access = node.accesskit_node();
        if access.is_hidden() {
            continue;
        }
        let Some(rect) = access_rect(&access) else {
            continue;
        };
        if has_text(&access) {
            assert!(
                rect.width() > 0.5 && rect.height() > 0.5,
                "{scene} has zero-size text node: {node:?}"
            );
        }
        let mut ancestor = Some(node);
        let mut enclosing_card = None;
        while let Some(candidate) = ancestor {
            let candidate_id = candidate.accesskit_node().id();
            if let Some(card) = cards.iter().find(|(_, id, _)| *id == candidate_id) {
                enclosing_card = Some(card);
                break;
            }
            ancestor = candidate.parent();
        }
        let Some((card, _, card_rect)) = enclosing_card else {
            continue;
        };
        assert!(
            raster_contains_rect(*card_rect, rect),
            "{scene} node overflows {card:?}: node={node:?}, card_rect={card_rect:?}"
        );
    }
}

fn raster_contains_rect(outer: egui::Rect, inner: egui::Rect) -> bool {
    crate::common::raster_pixel(inner.left()) >= crate::common::raster_pixel(outer.left())
        && crate::common::raster_pixel(inner.top()) >= crate::common::raster_pixel(outer.top())
        && crate::common::raster_pixel(inner.right()) <= crate::common::raster_pixel(outer.right())
        && crate::common::raster_pixel(inner.bottom())
            <= crate::common::raster_pixel(outer.bottom())
}

fn has_text(access: &AccessKitNode<'_>) -> bool {
    access.label().is_some_and(|text| !text.is_empty())
        || access.value().is_some_and(|text| !text.is_empty())
}

fn access_rect(access: &AccessKitNode<'_>) -> Option<egui::Rect> {
    let rect = access.bounding_box()?;
    Some(egui::Rect {
        min: egui::pos2(rect.x0 as f32, rect.y0 as f32),
        max: egui::pos2(rect.x1 as f32, rect.y1 as f32),
    })
}

fn card_node<'a>(
    harness: &'a Harness<'_, ParamExApp>,
    label: &'a str,
) -> Option<(egui_kittest::Node<'a>, egui::Rect)> {
    let title = harness.get_by_label(label);
    let title_rect = title.rect();
    let mut node = title;
    while let Some(parent) = node.parent() {
        let Some(rect) = access_rect(&parent.accesskit_node()) else {
            node = parent;
            continue;
        };
        if rect.width() >= title_rect.width() + 48.0 && rect.height() >= title_rect.height() + 48.0
        {
            return Some((parent, rect));
        }
        node = parent;
    }
    None
}

fn assert_tight_bottom_in_rect(
    scene: &str,
    harness: &Harness<'_, ParamExApp>,
    card: egui::Rect,
    terminal_label: &str,
) {
    let terminal = harness
        .get_all_by_label(terminal_label)
        .map(|node| node.rect())
        .filter(|rect| card.contains(rect.center()))
        .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .unwrap_or_else(|| {
            panic!("{scene} should expose terminal control {terminal_label:?} inside {card:?}")
        });
    let inner_bottom = card.bottom() - paramex_gui::ui_kit::CARD_INNER_MARGIN as f32;
    let bottom_slack =
        crate::common::raster_pixel(inner_bottom) - crate::common::raster_pixel(terminal.bottom());

    assert!(
        (0..=6).contains(&bottom_slack),
        "{scene} leaves {bottom_slack}px below {terminal_label:?}; expected 0-6 exact raster pixels: terminal={terminal:?}, inner_bottom={inner_bottom:.1}, card={card:?}"
    );
}
