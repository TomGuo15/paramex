//! Transfer file-list bulk action policy.

use egui_notify::Toasts;

use crate::format_ui::{cleared_error_rows, removed_items};
use crate::workspaces::transfer::state::PendingOutput;
use crate::workspaces::transfer::TransferWorkspace;

/// "Remove Checked", or "Remove Selected" when none are checked
/// (`file_list_panel.py:192-202`). The dynamic GUI label makes this policy explicit.
pub(super) fn remove_selected_or_checked(workspace: &mut TransferWorkspace, toasts: &mut Toasts) {
    let removed = workspace.remove_selected_or_checked();
    if removed == 0 {
        toasts.warning("No files selected to remove.");
    } else {
        toasts.info(removed_items(removed, "file"));
    }
}

/// "Keep Checked": remove the unchecked set (`file_list_panel.py:204-212`).
pub(super) fn keep_checked(workspace: &mut TransferWorkspace, toasts: &mut Toasts) {
    match workspace.keep_checked_files() {
        None => {
            toasts.warning("Check the files you want to keep first.");
        }
        Some(removed) if removed > 0 => {
            toasts.info(removed_items(removed, "file"));
        }
        Some(_) => {}
    }
}

/// "Clear All": remove every file (`file_list_panel.py:214-219`).
pub(super) fn clear_all(workspace: &mut TransferWorkspace, toasts: &mut Toasts) {
    let had_errors = workspace.file_rows.has_errors();
    let removed = workspace.clear_files();
    let pending = workspace.pending_outputs.len();
    workspace.pending_outputs.clear();
    if had_errors {
        workspace.file_rows.clear_errors();
    }
    if removed > 0 || had_errors || pending > 0 {
        if removed > 0 {
            toasts.info(removed_items(removed, "file"));
        } else if pending > 0 {
            toasts.info("Cleared pending output row(s).");
        } else {
            toasts.info(cleared_error_rows());
        }
    }
}

pub(super) fn attach_pending_output(workspace: &mut TransferWorkspace, pending_id: &str) -> bool {
    let Some(file_id) = workspace.session.active_file_id() else {
        return false;
    };
    let file_id = file_id.to_string();
    let Some(pos) = workspace
        .pending_outputs
        .iter()
        .position(|row| row.id() == pending_id)
    else {
        return false;
    };
    let pending = workspace.pending_outputs.remove(pos);
    let reason = pending.reason();
    let output = pending.into_dataset();
    match workspace.session.replace_output(&file_id, output) {
        Ok(displaced) => {
            if let Some(displaced) = displaced {
                workspace.retain_detached_output(displaced);
            }
            true
        }
        Err(output) => {
            workspace
                .pending_outputs
                .insert(pos, PendingOutput::new(output, reason));
            false
        }
    }
}

pub(super) fn detach_output_to_pending(workspace: &mut TransferWorkspace, file_id: &str) -> bool {
    if let Some(output) = workspace.session.take_output(file_id) {
        workspace.retain_detached_output(output);
        true
    } else {
        false
    }
}

pub(super) fn remove_attached_output(workspace: &mut TransferWorkspace, file_id: &str) -> bool {
    workspace.session.take_output(file_id).is_some()
}

pub(super) fn remove_pending_output(workspace: &mut TransferWorkspace, pending_id: &str) -> bool {
    let before = workspace.pending_outputs.len();
    workspace
        .pending_outputs
        .retain(|row| row.id() != pending_id);
    workspace.pending_outputs.len() != before
}
