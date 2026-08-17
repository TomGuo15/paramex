//! Right-column TLM FILES status card.

use eframe::egui;

use crate::table_kit;
use crate::ui_kit;
use crate::workspaces::tlm::panels::columns::STATUS_COLS;
use crate::workspaces::tlm::state::TlmState;

use super::grid::grid_table;

/// One file + status row per workbook, the always-visible replacement for the
/// removed Files tab. The header pill carries the failure count; a failed row's
/// parse message shows on status-cell hover. Each row carries a ✕ to drop that
/// workbook from the dataset; the clicked file (the `status.file` path in row 0)
/// is returned for the caller to remove + re-analyze.
#[must_use]
pub fn show_files(ui: &mut egui::Ui, tlm: &TlmState, actions_enabled: bool) -> Option<String> {
    let mut remove_file = None;
    ui_kit::card_slot(ui, |ui| {
        let card = tlm.files_card();
        let pill = card.map(|card| {
            if card.error_count > 0 {
                format!("{} failed", card.error_count)
            } else {
                format!("{} ok", card.status_count)
            }
        });
        ui_kit::section_header(ui, "FILES", pill.as_deref());

        // Fixed schema columns fill the card without reacting to filename length.
        // Horizontal-only outer scroll; the table owns vertical scroll with a
        // sticky header.
        let empty_rows: &[Vec<String>] = &[];
        let rows = if card.is_some() {
            tlm.rows().status()
        } else {
            empty_rows
        };
        let clicked = table_kit::horizontal_table_scroll(ui, "tlm_files_scroll", |ui, card_w| {
            grid_table(
                ui,
                "tlm_status_table",
                &STATUS_COLS,
                rows,
                card_w,
                tlm.rows_generation(),
                Some(actions_enabled),
            )
        });
        // Row 0 of a status row is the `status.file` path the reducer matches on.
        remove_file = clicked
            .and_then(|idx| rows.get(idx))
            .and_then(|r| r.first())
            .cloned();
    });
    remove_file
}
