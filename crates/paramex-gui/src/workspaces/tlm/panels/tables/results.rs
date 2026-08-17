//! Center TLM results card: result tabs plus CSV export actions.

use eframe::egui;

use crate::io_tasks::IoQueue;
use crate::table_kit;
use crate::ui_kit::{self, Variant};
use crate::workspaces::tlm::ingest::{start_export_tlm_csv, Msg};
use crate::workspaces::tlm::panels::columns::{LENGTH_COLS, RESULT_COLS, SWEEP_COLS};
use crate::workspaces::tlm::state::{TlmState, TlmTab};

use super::grid::grid_table;

pub(crate) fn show_results(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    tlm: &mut TlmState,
    io: &mut IoQueue<Msg>,
) {
    let card = tlm.results_card();
    let export_enabled = io.is_idle();
    let mut switch: Option<TlmTab> = None;

    ui_kit::card_slot(ui, |ui| {
        let labels = [
            "Group fits",
            "Fits vs V<sub>G</sub>",
            "R<sub>total</sub> points",
        ];
        let (clicked, (tlm_clicked, sweep_clicked)) = ui_kit::header_nav_action_row(
            ui,
            "RESULTS",
            |ui| {
                ui.add_enabled_ui(card.has_result, |ui| {
                    ui_kit::segmented(
                        ui,
                        &labels,
                        card.active_tab.index(),
                        ui_kit::SegStyle::Card,
                        None,
                    )
                })
                .inner
            },
            |ui| {
                let tlm_clicked = ui
                    .add_enabled_ui(card.has_result && export_enabled, |ui| {
                        ui_kit::header_action(ui, "Export TLM CSV", Variant::Primary).clicked()
                    })
                    .inner;
                let sweep_clicked = ui
                    .add_enabled_ui(card.has_sweep && export_enabled, |ui| {
                        ui_kit::header_action(ui, "Export Sweep CSV", Variant::Secondary).clicked()
                    })
                    .inner;
                (tlm_clicked, sweep_clicked)
            },
        );
        if tlm_clicked {
            if let Some(bytes) = tlm.result_csv_bytes() {
                start_export_tlm_csv(ctx, io, bytes, "paramex_tlm_result.csv");
            }
        }
        if sweep_clicked {
            if let Some(bytes) = tlm.sweep_csv_bytes() {
                start_export_tlm_csv(ctx, io, bytes, "paramex_tlm_sweep.csv");
            }
        }
        if let Some(idx) = clicked {
            switch = Some(TlmTab::from_index(idx));
        }

        // The id_salt intentionally includes the tab index so each tab gets its own
        // scroll memory. This causes the ScrollArea's rect to change Id on tab switch,
        // which would trip egui's debug `warn_if_rect_changes_id` overlay — harmless
        // because that overlay is globally silenced in `theme::install`.
        // Horizontal-only: the table owns vertical scroll (sticky header).
        let empty_rows: &[Vec<String>] = &[];
        table_kit::horizontal_table_scroll(
            ui,
            ("tlm_tab_scroll", card.active_tab.index()),
            |ui, card_w| match card.active_tab {
                // The analytical tables have no remove action (only the FILES card does),
                // so they pass `None` and ignore the returned index.
                TlmTab::Results => {
                    grid_table(
                        ui,
                        "tlm_result_table",
                        &RESULT_COLS,
                        if card.has_result {
                            tlm.rows().results()
                        } else {
                            empty_rows
                        },
                        card_w,
                        tlm.rows_generation(),
                        None,
                    );
                }
                // `has_sweep` stays the presence signal; the rows are the
                // reducer-built projection of that same sweep.
                TlmTab::Sweep => {
                    grid_table(
                        ui,
                        "tlm_sweep_table",
                        &SWEEP_COLS,
                        if card.has_sweep {
                            tlm.rows().sweep()
                        } else {
                            empty_rows
                        },
                        card_w,
                        tlm.rows_generation(),
                        None,
                    );
                }
                TlmTab::Lengths => {
                    grid_table(
                        ui,
                        "tlm_length_table",
                        &LENGTH_COLS,
                        if card.has_result {
                            tlm.rows().lengths()
                        } else {
                            empty_rows
                        },
                        card_w,
                        tlm.rows_generation(),
                        None,
                    );
                }
            },
        );
    });
    if let Some(tab) = switch {
        tlm.set_results_tab(tab);
    }
}
