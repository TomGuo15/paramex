//! DEVICES card: Transfer-style file selection, bulk actions, and output rows.

use eframe::egui;
use egui_notify::Toasts;

use crate::format_ui::{
    analog_fit_row_status, cleared_model_fit_rows, fmt_num3, removed_items, LOW_R2_THRESHOLD,
    OUTPUT_FIT_FAILED_MESSAGE, REMOVED_PENDING_OUTPUT_MESSAGE,
};
use crate::io_tasks::IoQueue;
use crate::state::EditBuffers;
use crate::ui_kit::{self, BadgeTone, OutputActionIcon, StatusLineText, Variant};
use crate::workspaces::modelfit::ingest::{start_dibl_refinement, start_output_refinement, Msg};
use crate::workspaces::modelfit::models::AOSTFT_INDEX;
use crate::workspaces::modelfit::state::{
    DiblRefinementPurpose, IngestIssues, ModelFitState, OutputRefinementPurpose,
};
use crate::workspaces::modelfit::ModelFitWorkspace;

#[derive(Default)]
struct RowActions {
    select: Option<usize>,
    toggle: Option<(usize, bool)>,
    remove_selected_or_checked: bool,
    keep_checked: bool,
    clear_all: bool,
    dismiss_error: Option<String>,
    attach_pending_output: Option<usize>,
    remove_pending_output: Option<usize>,
    attach_pending_dibl: Option<usize>,
    remove_pending_dibl: Option<usize>,
    detach_output: Option<usize>,
    detach_dibl: Option<usize>,
    remove_attached_output: Option<usize>,
    remove_attached_dibl: Option<usize>,
}

pub fn show(
    ui: &mut egui::Ui,
    workspace: &mut ModelFitWorkspace,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    actions_enabled: bool,
) {
    let ModelFitWorkspace {
        state, io, issues, ..
    } = workspace;
    let ctx = ui.ctx().clone();
    let actions = ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "DEVICES", None);
        let mut actions = render_management_actions(ui, state, issues, actions_enabled);
        ui.add_space(8.0);
        let rows_h = ui.available_height().max(0.0);
        let row_actions = render_rows(ui, state, issues, rows_h, actions_enabled);
        actions.select = row_actions.select;
        actions.toggle = row_actions.toggle;
        actions.dismiss_error = row_actions.dismiss_error;
        actions.attach_pending_output = row_actions.attach_pending_output;
        actions.remove_pending_output = row_actions.remove_pending_output;
        actions.attach_pending_dibl = row_actions.attach_pending_dibl;
        actions.remove_pending_dibl = row_actions.remove_pending_dibl;
        actions.detach_output = row_actions.detach_output;
        actions.detach_dibl = row_actions.detach_dibl;
        actions.remove_attached_output = row_actions.remove_attached_output;
        actions.remove_attached_dibl = row_actions.remove_attached_dibl;
        actions
    });

    apply_actions(actions, state, io, issues, edits, toasts, &ctx);
}

fn render_management_actions(
    ui: &mut egui::Ui,
    state: &ModelFitState,
    issues: &IngestIssues,
    actions_enabled: bool,
) -> RowActions {
    let mut actions = RowActions::default();
    let has_devices = !state.is_empty();
    let has_errors = issues.has_errors();
    let has_pending = !state.pending_outputs().is_empty() || !state.pending_dibls().is_empty();
    let has_checked = state.has_checked_devices();
    let can_keep_checked = has_checked && state.has_unchecked_devices();
    let remove_label = if has_checked {
        "Remove Checked"
    } else {
        "Remove Selected"
    };
    let can_clear = state.device_count() > 1 || has_errors || has_pending;

    ui.columns(2, |cols| {
        actions.remove_selected_or_checked = cols[0]
            .add_enabled_ui(actions_enabled && has_devices, |ui| {
                ui_kit::button_full(ui, remove_label, Variant::Danger)
            })
            .inner
            .clicked();
        actions.clear_all = cols[1]
            .add_enabled_ui(actions_enabled && can_clear, |ui| {
                ui_kit::button_full(ui, "Clear All", Variant::Danger)
            })
            .inner
            .clicked();
    });
    ui.add_space(4.0);
    actions.keep_checked = ui
        .add_enabled_ui(actions_enabled && can_keep_checked, |ui| {
            ui_kit::button_full(ui, "Keep Checked", Variant::Secondary)
        })
        .inner
        .clicked();
    actions
}

fn render_rows(
    ui: &mut egui::Ui,
    state: &ModelFitState,
    issues: &IngestIssues,
    rows_h: f32,
    actions_enabled: bool,
) -> RowActions {
    let selected = state.selected_index();
    let mut actions = RowActions::default();
    ui_kit::scroll_body(ui, "modelfit_device_rows", rows_h, |ui| {
        for (idx, entry) in state.devices().iter().enumerate() {
            let device = entry.device();
            let is_selected = selected == Some(idx);
            let device_id = entry.id();
            let row_id = ui.id().with(("modelfit_device_row", device_id));
            let hovered = ui
                .ctx()
                .read_response(row_id)
                .map(|response| response.hovered())
                .unwrap_or(false);
            let frame = ui_kit::selection_row_frame(ui, is_selected, hovered);
            let mut checkbox_rect = egui::Rect::NOTHING;
            let inner = frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let mut checked = entry.is_checked();
                    let checkbox = ui
                        .push_id(("modelfit_device_check", device_id), |ui| {
                            ui.checkbox(&mut checked, "")
                        })
                        .inner;
                    let enabled = checkbox.enabled();
                    let label = format!("Mark {} for bulk actions", device.name());
                    checkbox.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            enabled,
                            checked,
                            label.clone(),
                        )
                    });
                    let checkbox = checkbox.on_hover_text(label);
                    checkbox_rect = checkbox.rect;
                    if checkbox.changed() {
                        actions.toggle = Some((idx, checked));
                    }

                    let model = device.model(state.selected_fit_model());
                    let analog = model.analog_fit_quality();
                    let analog_status = analog_fit_row_status(
                        analog.gm_p90,
                        analog.gds_p90,
                        device.has_output_curves(),
                    );
                    let (badge, tone, status) = match model.r2() {
                        Some(r2) if r2 < LOW_R2_THRESHOLD => (
                            "warn",
                            BadgeTone::Warning,
                            format!("transfer R\u{00B2} {}", fmt_num3(r2)),
                        ),
                        Some(_)
                            if state.selected_model() == AOSTFT_INDEX
                                && device.has_output_curves()
                                && !device.has_output() =>
                        {
                            (
                                "warn",
                                BadgeTone::Warning,
                                OUTPUT_FIT_FAILED_MESSAGE.to_string(),
                            )
                        }
                        Some(_) if analog_status.is_some() => (
                            "warn",
                            BadgeTone::Warning,
                            analog_status.expect("checked analog warning"),
                        ),
                        Some(r2) => (
                            "ok",
                            BadgeTone::Ok,
                            format!("transfer R\u{00B2} {}", fmt_num3(r2)),
                        ),
                        None => ("warn", BadgeTone::Warning, "No fit".to_string()),
                    };
                    ui_kit::list_row_title_status(
                        ui,
                        device.name(),
                        badge,
                        tone,
                        StatusLineText::Inline(status.as_str()),
                        |_| {},
                    );
                });
            });
            if is_selected {
                ui_kit::selection_bar(ui, inner.response.rect);
            }
            let row_rect = inner.response.rect;
            let select_rect = egui::Rect::from_min_max(
                egui::pos2(checkbox_rect.right().min(row_rect.right()), row_rect.top()),
                row_rect.max,
            );
            if ui_kit::selectable_row_response(ui, select_rect, row_id, device.name(), is_selected)
                .clicked()
            {
                actions.select = Some(idx);
            }

            if let Some(output_name) = entry.output_name() {
                render_attached_output_row(ui, idx, output_name, actions_enabled, &mut actions);
            }
            if let Some(dibl_name) = entry.dibl_name() {
                render_attached_dibl_row(
                    ui,
                    idx,
                    dibl_name,
                    entry.device().is_second_transfer_applied(),
                    actions_enabled,
                    &mut actions,
                );
            }
        }

        for (idx, pending) in state.pending_outputs().iter().enumerate() {
            render_pending_output_row(
                ui,
                idx,
                pending.name(),
                pending.reason().label(),
                actions_enabled,
                actions_enabled && selected.is_some(),
                &mut actions,
            );
        }

        for (idx, pending) in state.pending_dibls().iter().enumerate() {
            render_pending_dibl_row(
                ui,
                idx,
                pending.name(),
                pending.reason().label(),
                actions_enabled,
                actions_enabled && selected.is_some() && state.selected_model_is_level62(),
                &mut actions,
            );
        }

        for row in issues.rows() {
            if ui_kit::file_error_row(ui, row.name, row.message) {
                actions.dismiss_error = Some(row.id.to_string());
            }
        }
    });
    actions
}

fn render_attached_output_row(
    ui: &mut egui::Ui,
    device_idx: usize,
    output_name: &str,
    actions_enabled: bool,
    actions: &mut RowActions,
) {
    ui_kit::selection_row_frame(ui, false, false).show(ui, |ui| {
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
                        actions.remove_attached_output = Some(device_idx);
                    }
                    if ui_kit::output_action_icon_button(
                        ui,
                        "Detach output",
                        OutputActionIcon::Detach,
                    )
                    .on_hover_text("Move this output to pending")
                    .clicked()
                    {
                        actions.detach_output = Some(device_idx);
                    }
                });
            });
        });
    });
}

fn render_attached_dibl_row(
    ui: &mut egui::Ui,
    device_idx: usize,
    dibl_name: &str,
    is_applied: bool,
    actions_enabled: bool,
    actions: &mut RowActions,
) {
    ui_kit::selection_row_frame(ui, false, false).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::file_row_gutter(ui);
            ui.vertical(|ui| {
                let (tone, status) = if is_applied {
                    (BadgeTone::Ok, "applied")
                } else {
                    (BadgeTone::Warning, "retained · inactive in manual mode")
                };
                ui_kit::list_row_title_status(
                    ui,
                    dibl_name,
                    "DIBL",
                    tone,
                    StatusLineText::Inline(status),
                    |_| {},
                );
            });
            ui_kit::right_aligned(ui, |ui| {
                ui.add_enabled_ui(actions_enabled, |ui| {
                    if ui_kit::close_button(ui, "Remove attached DIBL")
                        .on_hover_text("Remove this attached DIBL measurement")
                        .clicked()
                    {
                        actions.remove_attached_dibl = Some(device_idx);
                    }
                    if ui_kit::output_action_icon_button(
                        ui,
                        "Detach DIBL",
                        OutputActionIcon::Detach,
                    )
                    .on_hover_text("Move this DIBL measurement to pending")
                    .clicked()
                    {
                        actions.detach_dibl = Some(device_idx);
                    }
                });
            });
        });
    });
}

fn render_pending_output_row(
    ui: &mut egui::Ui,
    pending_idx: usize,
    name: &str,
    reason: &str,
    actions_enabled: bool,
    can_attach: bool,
    actions: &mut RowActions,
) {
    ui_kit::selection_row_frame(ui, false, false).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::file_row_gutter(ui);
            ui.vertical(|ui| {
                ui_kit::list_row_title_status(
                    ui,
                    name,
                    "pending",
                    BadgeTone::Warning,
                    StatusLineText::Inline(reason),
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
                    actions.remove_pending_output = Some(pending_idx);
                }
                if ui
                    .add_enabled_ui(can_attach, |ui| {
                        ui_kit::output_action_icon_button(
                            ui,
                            "Attach to Selected",
                            OutputActionIcon::Attach,
                        )
                        .on_hover_text("Attach to selected transfer file")
                        .clicked()
                    })
                    .inner
                {
                    actions.attach_pending_output = Some(pending_idx);
                }
            });
        });
    });
}

fn render_pending_dibl_row(
    ui: &mut egui::Ui,
    pending_idx: usize,
    name: &str,
    reason: &str,
    actions_enabled: bool,
    can_attach: bool,
    actions: &mut RowActions,
) {
    ui_kit::selection_row_frame(ui, false, false).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui_kit::file_row_gutter(ui);
            ui.vertical(|ui| {
                ui_kit::list_row_title_status(
                    ui,
                    name,
                    "pending",
                    BadgeTone::Warning,
                    StatusLineText::Inline(reason),
                    |_| {},
                );
            });
            ui_kit::right_aligned(ui, |ui| {
                if ui
                    .add_enabled_ui(actions_enabled, |ui| {
                        ui_kit::close_button(ui, "Remove pending DIBL")
                            .on_hover_text("Remove this pending DIBL row")
                            .clicked()
                    })
                    .inner
                {
                    actions.remove_pending_dibl = Some(pending_idx);
                }
                if ui
                    .add_enabled_ui(can_attach, |ui| {
                        ui_kit::output_action_icon_button(
                            ui,
                            "Attach DIBL to Selected",
                            OutputActionIcon::Attach,
                        )
                        .on_hover_text("Attach DIBL to selected transfer file")
                        .clicked()
                    })
                    .inner
                {
                    actions.attach_pending_dibl = Some(pending_idx);
                }
            });
        });
    });
}

fn apply_actions(
    actions: RowActions,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
    issues: &mut IngestIssues,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    ctx: &egui::Context,
) {
    if actions.clear_all {
        let devices = state.device_count();
        let pending = state.pending_outputs().len() + state.pending_dibls().len();
        let errors = issues.clear();
        state.clear();
        forget_device_edits(edits);
        if devices + pending + errors > 0 {
            toasts.info(cleared_model_fit_rows(devices, pending, errors));
        }
        return;
    }
    if actions.remove_selected_or_checked {
        let removed = state.remove_selected_or_checked();
        if removed > 0 {
            forget_device_edits(edits);
            toasts.info(removed_items(removed, "device"));
        } else {
            toasts.warning("No devices selected to remove.");
        }
        return;
    }
    if actions.keep_checked {
        match state.keep_checked_devices() {
            Some(removed) if removed > 0 => {
                forget_device_edits(edits);
                toasts.info(removed_items(removed, "device"));
            }
            None => {
                toasts.warning("Check the devices you want to keep first.");
            }
            Some(_) => {}
        }
        return;
    }
    if let Some(idx) = actions.remove_attached_output {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_output_clear(idx, OutputRefinementPurpose::Remove) {
            start_output_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.remove_attached_dibl {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_dibl_clear(idx, DiblRefinementPurpose::Remove) {
            start_dibl_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.detach_dibl {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_dibl_clear(idx, DiblRefinementPurpose::Detach) {
            start_dibl_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.detach_output {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_output_clear(idx, OutputRefinementPurpose::Detach) {
            start_output_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.attach_pending_output {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_pending_output_attach(idx) {
            start_output_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.attach_pending_dibl {
        edits.forget_prefix("modelfit:p:");
        if let Some(plan) = state.plan_pending_dibl_attach(idx) {
            start_dibl_refinement(ctx, io, plan);
        }
        return;
    }
    if let Some(idx) = actions.remove_pending_output {
        if state.remove_pending_output(idx) {
            toasts.info(REMOVED_PENDING_OUTPUT_MESSAGE);
        }
        return;
    }
    if let Some(idx) = actions.remove_pending_dibl {
        if state.remove_pending_dibl(idx) {
            toasts.info("Removed pending DIBL.");
        }
        return;
    }
    if let Some(id) = actions.dismiss_error {
        issues.dismiss(&id);
    }
    if let Some((idx, checked)) = actions.toggle {
        state.set_device_checked(idx, checked);
    }
    if let Some(idx) = actions.select {
        if state.selected_index() != Some(idx) {
            forget_device_edits(edits);
        }
        state.select(idx);
    }
}

fn forget_device_edits(edits: &mut EditBuffers) {
    edits.forget_prefix("modelfit:p:");
    edits.forget_prefix("modelfit:geom:");
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        kittest::{NodeT, Queryable},
        Harness,
    };
    use paramex_core::modelfit::{FittedDevice, SecondTransfer};

    use super::*;
    use crate::workspaces::modelfit::state::{
        run_dibl_refinement, DeviceInstallOutcome, DiblImport, DiblSource, PrimaryTransferSource,
    };

    struct DevicesTestApp {
        workspace: ModelFitWorkspace,
        edits: EditBuffers,
        toasts: Toasts,
        actions_enabled: bool,
    }

    impl eframe::App for DevicesTestApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.workspace.drain_ingest(ui.ctx(), &mut self.toasts);
            let actions_enabled = self.actions_enabled && self.workspace.is_idle();
            show(
                ui,
                &mut self.workspace,
                &mut self.edits,
                &mut self.toasts,
                actions_enabled,
            );
        }
    }

    fn pointer_press_and_release_at(harness: &mut Harness<'_, DevicesTestApp>, pos: egui::Pos2) {
        harness.hover_at(pos);
        harness.step();
        harness.drag_at(pos);
        harness.step();
        harness.drop_at(pos);
        harness.step();
    }

    fn wait_for_ingest(harness: &mut Harness<'_, DevicesTestApp>) {
        for _ in 0..2_000 {
            harness.step();
            if harness.state().workspace.is_idle() {
                harness.step();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("timed out waiting for the DIBL refinement worker");
    }

    fn fitted_state_with_attached_and_pending_dibl() -> ModelFitState {
        let vg = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
        let transfer = |vt: f64| {
            vg.iter()
                .map(|&gate| {
                    let overdrive = gate - vt;
                    if overdrive > 0.0 {
                        1.0e-6 * overdrive.powf(1.5)
                    } else {
                        1.0e-12
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut state = ModelFitState::default();
        assert_eq!(
            state
                .install_fitted_device(
                    FittedDevice::fit("device.csv".into(), vg.clone(), transfer(1.0)).unwrap(),
                    PrimaryTransferSource::new("device.csv", None).unwrap(),
                    None,
                )
                .unwrap(),
            DeviceInstallOutcome::Installed
        );
        assert!(state.set_selected_model(crate::workspaces::modelfit::models::LEVEL62_INDEX));
        for name in ["low-a.csv", "low-b.csv"] {
            let plan = state.plan_dibl_refinement(
                vec![DiblImport {
                    source: DiblSource::new(name, None).unwrap(),
                    second: SecondTransfer {
                        vg: vg.clone(),
                        id_abs: transfer(1.5),
                        v_ds: 1.0,
                    },
                }],
                state.selected_device_id(),
                true,
                Vec::new(),
            );
            state.commit_dibl_refinement(run_dibl_refinement(plan));
        }
        state
    }

    #[test]
    fn dibl_row_distinguishes_applied_from_manual_inactive_retention() {
        let mut snapshot_results = egui_kittest::SnapshotResults::new();
        let state = fitted_state_with_attached_and_pending_dibl();
        assert!(state
            .selected_entry()
            .unwrap()
            .device()
            .is_second_transfer_applied());
        let app = DevicesTestApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
            actions_enabled: true,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 460.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();
        harness.get_by_label("applied");
        harness.snapshot("modelfit_devices_dibl_applied");
        snapshot_results.extend_harness(&mut harness);

        let mut state = fitted_state_with_attached_and_pending_dibl();
        let params = state
            .selected_entry()
            .unwrap()
            .device()
            .level62()
            .expect("fixture has Level 62")
            .params;
        state
            .set_selected_level62_params(params)
            .expect("same valid values enter manual mode");
        assert!(!state
            .selected_entry()
            .unwrap()
            .device()
            .is_second_transfer_applied());
        let app = DevicesTestApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
            actions_enabled: true,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 460.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();
        harness.get_by_label("retained · inactive in manual mode");
        harness.snapshot("modelfit_devices_dibl_manual_inactive");
        snapshot_results.extend_harness(&mut harness);
    }

    #[test]
    fn dibl_row_actions_are_pointer_inert_while_actions_are_disabled() {
        let state = fitted_state_with_attached_and_pending_dibl();
        assert_eq!(
            state.selected_entry().unwrap().dibl_name(),
            Some("low-b.csv")
        );
        assert_eq!(state.pending_dibls().len(), 1);
        let app = DevicesTestApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
            actions_enabled: false,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 460.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        for label in [
            "Remove attached DIBL",
            "Detach DIBL",
            "Remove pending DIBL",
            "Attach DIBL to Selected",
        ] {
            assert!(
                harness.get_by_label(label).accesskit_node().is_disabled(),
                "{label} should be disabled while another operation is running"
            );
            harness.get_by_label(label).click();
            harness.run();
        }

        assert_eq!(
            harness
                .state()
                .workspace
                .state()
                .selected_entry()
                .unwrap()
                .dibl_name(),
            Some("low-b.csv")
        );
        assert_eq!(harness.state().workspace.state().pending_dibls().len(), 1);
    }

    #[test]
    fn enabled_pointer_actions_detach_then_reattach_dibl() {
        let state = fitted_state_with_attached_and_pending_dibl();
        let app = DevicesTestApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
            actions_enabled: true,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 460.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        let detach_pos = harness.get_by_label("Detach DIBL").rect().center();
        pointer_press_and_release_at(&mut harness, detach_pos);
        wait_for_ingest(&mut harness);

        let state = harness.state().workspace.state();
        assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
        assert_eq!(state.pending_dibls().len(), 2);

        let attach_pos = harness
            .get_all_by_label("Attach DIBL to Selected")
            .last()
            .expect("detached DIBL should render an attach action")
            .rect()
            .center();
        pointer_press_and_release_at(&mut harness, attach_pos);
        wait_for_ingest(&mut harness);

        let state = harness.state().workspace.state();
        assert_eq!(
            state.selected_entry().unwrap().dibl_name(),
            Some("low-b.csv")
        );
        assert_eq!(state.pending_dibls().len(), 1);
        assert_eq!(state.pending_dibls()[0].name(), "low-a.csv");
    }

    #[test]
    fn enabled_pointer_remove_actions_are_destructive_and_row_scoped() {
        let state = fitted_state_with_attached_and_pending_dibl();
        let app = DevicesTestApp {
            workspace: ModelFitWorkspace::from_state(state),
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
            actions_enabled: true,
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 460.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        let remove_attached_pos = harness.get_by_label("Remove attached DIBL").rect().center();
        pointer_press_and_release_at(&mut harness, remove_attached_pos);
        wait_for_ingest(&mut harness);

        let state = harness.state().workspace.state();
        assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
        assert_eq!(state.pending_dibls().len(), 1);
        assert_eq!(state.pending_dibls()[0].name(), "low-a.csv");

        let remove_pending_pos = harness.get_by_label("Remove pending DIBL").rect().center();
        pointer_press_and_release_at(&mut harness, remove_pending_pos);

        let state = harness.state().workspace.state();
        assert_eq!(state.device_count(), 1);
        assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
        assert!(state.pending_dibls().is_empty());
    }
}
