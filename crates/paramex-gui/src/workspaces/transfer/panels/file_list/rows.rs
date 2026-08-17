//! Transfer file-list row rendering and deferred row actions.

use eframe::egui;
use paramex_core::transfer::{FileListRow, Session};

use crate::format_ui::{
    point_count_label, ATTACHED_PENDING_OUTPUT_MESSAGE, OUTPUT_MOVED_TO_PENDING_MESSAGE,
    REMOVED_OUTPUT_MESSAGE, REMOVED_PENDING_OUTPUT_MESSAGE,
};
use crate::ui_kit::{self, output_action_icon_button, BadgeTone, OutputActionIcon, StatusLineText};
use crate::workspaces::transfer::state::{FileRow, FileRows, PendingOutput};
use crate::workspaces::transfer::TransferWorkspace;

use super::model;

#[derive(Default)]
pub(super) struct RowActions {
    select: Option<String>,
    toggle: Option<(String, bool)>,
    dismiss_error: Option<String>,
    attach_pending_output: Option<String>,
    remove_pending_output: Option<String>,
    detach_output: Option<String>,
    remove_attached_output: Option<String>,
}

impl RowActions {
    pub(super) fn apply(self, workspace: &mut TransferWorkspace, toasts: &mut egui_notify::Toasts) {
        if let Some(id) = self.select {
            workspace.select_file(&id);
        }
        if let Some((id, checked)) = self.toggle {
            workspace.set_file_checked(&id, checked);
        }
        if let Some(id) = self.dismiss_error {
            workspace.file_rows.dismiss_error(&id);
        }
        if let Some(id) = self.attach_pending_output {
            if model::attach_pending_output(workspace, &id) {
                toasts.success(ATTACHED_PENDING_OUTPUT_MESSAGE);
            } else {
                toasts.warning("Select a transfer file before attaching output.");
            }
        }
        if let Some(id) = self.remove_pending_output {
            if model::remove_pending_output(workspace, &id) {
                toasts.info(REMOVED_PENDING_OUTPUT_MESSAGE);
            }
        }
        if let Some(id) = self.detach_output {
            if model::detach_output_to_pending(workspace, &id) {
                toasts.info(OUTPUT_MOVED_TO_PENDING_MESSAGE);
            }
        }
        if let Some(id) = self.remove_attached_output {
            if model::remove_attached_output(workspace, &id) {
                toasts.info(REMOVED_OUTPUT_MESSAGE);
            }
        }
    }
}

pub(super) fn render_rows(
    ui: &mut egui::Ui,
    session: &Session,
    file_rows: &FileRows,
    pending_outputs: &[PendingOutput],
    actions_enabled: bool,
) -> RowActions {
    let mut actions = RowActions::default();

    for row in file_rows.rows() {
        match row {
            FileRow::File { id } => {
                let Some(file_row) = session.file_list_row(id) else {
                    continue;
                };
                render_file_row(ui, &file_row, &mut actions);
                if let Some(output_name) = &file_row.output_name {
                    render_attached_output_row(
                        ui,
                        &file_row.file_id,
                        output_name,
                        actions_enabled,
                        &mut actions,
                    );
                }
            }
            FileRow::Error { id, name, message } => {
                render_error_row(ui, id, name, message, &mut actions);
            }
        }
    }
    for pending in pending_outputs {
        render_pending_output_row(
            ui,
            pending,
            actions_enabled,
            session.has_selected_file(),
            &mut actions,
        );
    }

    actions
}

fn render_file_row(ui: &mut egui::Ui, row: &FileListRow, actions: &mut RowActions) {
    // Row click target: render the row inside a frame, then `interact`
    // over the frame rect excluding the checkbox column so the rest of the row
    // selects the file while the checkbox keeps its own click handling.
    let row_id = ui.id().with(("file_row", row.file_id.as_str()));
    let hovered = ui
        .ctx()
        .read_response(row_id)
        .map(|r| r.hovered())
        .unwrap_or(false);
    let frame = ui_kit::selection_row_frame(ui, row.is_selected, hovered);
    let mut cb_rect = egui::Rect::NOTHING;
    let inner = frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let mut checked = row.is_checked;
            let cb = ui
                .push_id(("file_cb", row.file_id.as_str()), |ui| {
                    ui.checkbox(&mut checked, "")
                })
                .inner;
            let enabled = cb.enabled();
            let bulk_label = format!("Mark {} for bulk actions", row.name);
            cb.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Checkbox,
                    enabled,
                    checked,
                    bulk_label.clone(),
                )
            });
            let cb = cb.on_hover_text(bulk_label);
            cb_rect = cb.rect;
            if cb.changed() {
                actions.toggle = Some((row.file_id.clone(), checked));
            }
            render_compact_file_row_content(ui, row);
        });
    });

    let row_rect = inner.response.rect;
    if row.is_selected {
        ui_kit::selection_bar(ui, row_rect);
    }
    let select_rect = egui::Rect::from_min_max(
        egui::pos2(cb_rect.right().min(row_rect.right()), row_rect.top()),
        row_rect.max,
    );
    let response =
        ui_kit::selectable_row_response(ui, select_rect, row_id, &row.name, row.is_selected);
    if response.clicked() {
        actions.select = Some(row.file_id.clone());
    }
}

fn render_compact_file_row_content(ui: &mut egui::Ui, row: &FileListRow) {
    let points = point_count_label(row.point_count);
    ui_kit::list_row_title_status(
        ui,
        &row.name,
        "ok",
        BadgeTone::Ok,
        StatusLineText::Inline(points.as_str()),
        |ui| {
            if row.manual_ranges {
                ui_kit::semantic_badge(ui, "manual", BadgeTone::Warning);
            }
        },
    );
}

fn render_attached_output_row(
    ui: &mut egui::Ui,
    file_id: &str,
    output_name: &str,
    actions_enabled: bool,
    actions: &mut RowActions,
) {
    let frame = ui_kit::selection_row_frame(ui, false, false);
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::file_row_gutter(ui);
            ui.vertical(|ui| {
                ui_kit::list_row_title_status(
                    ui,
                    output_name,
                    "output",
                    BadgeTone::Ok,
                    StatusLineText::Inline("attached"),
                    |_| {},
                );
            });
            ui_kit::right_aligned(ui, |ui| {
                ui.add_enabled_ui(actions_enabled, |ui| {
                    if ui_kit::close_button(ui, "Remove attached output")
                        .on_hover_text("Remove this attached output")
                        .clicked()
                    {
                        actions.remove_attached_output = Some(file_id.to_string());
                    }
                    if output_action_icon_button(ui, "Detach output", OutputActionIcon::Detach)
                        .on_hover_text("Move this output to pending")
                        .clicked()
                    {
                        actions.detach_output = Some(file_id.to_string());
                    }
                });
            });
        });
    });
}

fn render_error_row(
    ui: &mut egui::Ui,
    id: &str,
    name: &str,
    message: &str,
    actions: &mut RowActions,
) {
    if ui_kit::file_error_row(ui, name, message) {
        actions.dismiss_error = Some(id.to_string());
    }
}

fn render_pending_output_row(
    ui: &mut egui::Ui,
    pending: &PendingOutput,
    actions_enabled: bool,
    can_attach: bool,
    actions: &mut RowActions,
) {
    let frame = ui_kit::selection_row_frame(ui, false, false);
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::file_row_gutter(ui);
            ui.vertical(|ui| {
                ui_kit::list_row_title_status(
                    ui,
                    pending.name(),
                    "pending",
                    BadgeTone::Warning,
                    StatusLineText::Inline(pending.reason().label()),
                    |_| {},
                );
            });
            ui_kit::right_aligned(ui, |ui| {
                if ui
                    .add_enabled_ui(actions_enabled, |ui| {
                        ui_kit::close_button(ui, "Remove pending output")
                            .on_hover_text("Remove this pending output row")
                            .clicked()
                    })
                    .inner
                {
                    actions.remove_pending_output = Some(pending.id().to_string());
                }
                if ui
                    .add_enabled_ui(actions_enabled && can_attach, |ui| {
                        output_action_icon_button(
                            ui,
                            "Attach to Selected",
                            OutputActionIcon::Attach,
                        )
                        .on_hover_text("Attach to selected transfer file")
                        .clicked()
                    })
                    .inner
                {
                    actions.attach_pending_output = Some(pending.id().to_string());
                }
            });
        });
    });
}
