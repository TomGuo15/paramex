//! The central two-graph window selector. Thin view over core seams;
//! commits windows through the deferred-recompute pattern (geometry.rs template).
//!
//! This module owns the public `show` entry point; separable concerns live in
//! private siblings: `window` (mode/window policy), `render` (plot + controls
//! pipeline), `drag` (on-chart hit-testing), and `controls` (per-graph inputs).
mod controls;
mod drag;
mod render;
mod window;

pub mod bands;
pub mod graph;
pub mod strip;

pub use window::{backward_display, derive_mode, GraphSide};

use eframe::egui;
use paramex_core::transfer::{ExpertRanges, ExpertWindow, Session};

use crate::state::EditBuffers;
use crate::ui_kit::{self, Variant};
use crate::workspaces::transfer::state::{GraphMode, PlotCache, PlotKind, SelectorUi};

use drag::Commit;
use render::{forget_selector_buffers, render_selector_column};

fn show_selector_pair(ui: &mut egui::Ui, mut add: impl FnMut(&mut egui::Ui, GraphSide)) {
    if super::plot_pair_should_stack(ui.available_size()) {
        let height = ui.available_height();
        super::show_stacked_plot_pair(ui, "selector_plot_pair", height, |ui, index| {
            add(
                ui,
                if index == 0 {
                    GraphSide::Vt
                } else {
                    GraphSide::Ss
                },
            );
        });
    } else {
        ui.columns(2, |cols| {
            add(&mut cols[0], GraphSide::Vt);
            add(&mut cols[1], GraphSide::Ss);
        });
    }
}

/// `Reset to Auto` is authoritative for the selected file. Clicking the header
/// button steals focus from any open V_G min/max field, whose `lost_focus` then
/// commits a `Window` pin LATER in `commits` than the `AutoFitAll` pushed for the
/// reset — and that field re-seeds from the NOT-yet-cleared window (the clear is
/// deferred to the APPLY phase), so it would re-pin exactly what the reset clears
/// and the reset would be silently undone. Make the reset win: drop any same-file
/// `Window` collected in the same frame. (Unlike the direction toggle — which
/// changes mode immediately, so forgetting the buffer re-seeds to the new
/// direction — here the buffer re-seeds to the stale window, so the stray commit
/// must be dropped, not just the buffer forgotten.)
fn drop_windows_superseded_by_reset(commits: &mut Vec<Commit>) {
    let reset_ids: Vec<String> = commits
        .iter()
        .filter_map(|c| match c {
            Commit::AutoFitAll { id } => Some(id.clone()),
            _ => None,
        })
        .collect();
    if reset_ids.is_empty() {
        return;
    }
    commits.retain(|c| !matches!(c, Commit::Window { id, .. } if reset_ids.contains(id)));
}

/// Render the window selector for the selected file.
#[allow(clippy::too_many_arguments)] // reason: disjoint sub-state the panel's full scope requires
pub fn show(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    session: &mut Session,
    sel: &mut SelectorUi,
    plot: &mut PlotCache,
    edits: &mut EditBuffers,
) {
    ui_kit::card_slot(ui, |ui| {
        let selected = session.selected_fit_window_file();
        let manual_range = selected
            .map(|selected| has_manual_expert_range(selected.expert_ranges))
            .unwrap_or(false);
        let reset_clicked = ui_kit::header_action_row(ui, "FIT", |ui| {
            ui.add_enabled_ui(manual_range, |ui| {
                ui_kit::header_action(ui, "Reset to Auto", Variant::Secondary).clicked()
            })
            .inner
        });
        // Self-healing cache prune: drop entries for files removed since last frame (D-6d-6).
        plot.prune_to(|id| session.has_file(id));
        // The selected-file view borrows only immutable selector inputs. Its last
        // use is the plot.view() call below, so the &session borrow ends before
        // the deferred APPLY phase mutates window state.
        if let Some(selected) = selected {
            let id = selected.file_id.to_string();
            let er = selected.expert_ranges;

            // Re-derive per-graph mode + forget stale buffers on a file switch (D-6d-4).
            if sel.sync_file(
                &id,
                derive_mode(er.vt_range, er.vt_range_bwd),
                derive_mode(er.ss_range, er.ss_range_bwd),
            ) {
                forget_selector_buffers(edits, &id);
            }

            // has_bwd and windows come from the committed result, matching the
            // displayed split whenever a result exists.
            let has_bwd = selected.has_backward_sweep;
            let (vt_w, ss_w, vt_wb, ss_wb) = (
                selected.vt_window,
                selected.ss_window,
                selected.vt_window_bwd,
                selected.ss_window_bwd,
            );

            // Per-file derived data (split sweeps, scatters, axes, fitters), cached once
            // per curve and borrowed immutably for the whole render phase.
            let view = plot.view(&id, selected.vg, selected.id_abs);
            let axes = view.axes();

            let vt_live = sel.live_window(PlotKind::Vt);
            let ss_live = sel.live_window(PlotKind::Ss);
            // Snapshot the drag state BEFORE the columns run `grab_band` (which mutates it):
            // the bands are drawn from this, and the strips read it too, so they stay in sync.
            let drag_at_start = sel.drag();

            // COLLECT: gather all commits during the render phase (deferred pattern).
            let mut commits: Vec<Commit> = Vec::new();

            if reset_clicked {
                commits.push(Commit::AutoFitAll { id: id.clone() });
            }

            // Each responsive graph group is the plot, its on-chart band grab, then controls.
            // Normal windows stay side by side; tall/narrow bodies stack VTH above SS.
            show_selector_pair(ui, |ui, side| {
                let (kind, plot_id, y_bounds, window, window_bwd, live) = match side {
                    GraphSide::Vt => (
                        PlotKind::Vt,
                        "selector_vt",
                        axes.vt_y(),
                        vt_w,
                        vt_wb,
                        vt_live,
                    ),
                    GraphSide::Ss => (
                        PlotKind::Ss,
                        "selector_ss",
                        axes.ss_y(),
                        ss_w,
                        ss_wb,
                        ss_live,
                    ),
                };
                render_selector_column(
                    ui,
                    view,
                    sel,
                    edits,
                    &mut commits,
                    &id,
                    side,
                    kind,
                    plot_id,
                    axes,
                    y_bounds,
                    window,
                    window_bwd,
                    live,
                    has_bwd,
                    true,
                    drag_at_start,
                );
            });

            // A focus-stolen field can append a Window pin that would undo a same-frame
            // Reset to Auto; let the reset win before applying.
            drop_windows_superseded_by_reset(&mut commits);

            // APPLY: now the snapshot borrow is released and we can mutate session.
            for c in commits {
                match c {
                    Commit::Window { id, which, win } => {
                        session.set_expert_window(&id, which, win);
                        // A manual window pins that graph's mode to the edited direction
                        // (so an edit made while in Auto flips the radio to Fwd/Bwd).
                        match which {
                            ExpertWindow::FwdVt => sel.set_mode(PlotKind::Vt, GraphMode::Fwd),
                            ExpertWindow::BwdVt => sel.set_mode(PlotKind::Vt, GraphMode::Bwd),
                            ExpertWindow::FwdSs => sel.set_mode(PlotKind::Ss, GraphMode::Fwd),
                            ExpertWindow::BwdSs => sel.set_mode(PlotKind::Ss, GraphMode::Bwd),
                        }
                    }
                    Commit::AutoFitAll { id } => {
                        session.clear_expert_windows(&id);
                        sel.reset_modes_to_auto();
                    }
                }
                // NO plot.invalidate here: windows are curve-independent (D-6d-6). Cache stays valid.
            }
        } else {
            let view = plot.view("__empty_selector__", &[], &[]);
            let axes = view.axes();
            let mut commits = Vec::new();
            ui.add_enabled_ui(false, |ui| {
                show_selector_pair(ui, |ui, side| {
                    let (kind, plot_id, y_bounds) = match side {
                        GraphSide::Vt => (PlotKind::Vt, "selector_vt", axes.vt_y()),
                        GraphSide::Ss => (PlotKind::Ss, "selector_ss", axes.ss_y()),
                    };
                    render_selector_column(
                        ui,
                        view,
                        sel,
                        edits,
                        &mut commits,
                        "__empty_selector__",
                        side,
                        kind,
                        plot_id,
                        axes,
                        y_bounds,
                        None,
                        None,
                        None,
                        true,
                        false,
                        None,
                    );
                });
            });
        }
    });
}

fn has_manual_expert_range(er: ExpertRanges) -> bool {
    er.vt_range.is_some()
        || er.ss_range.is_some()
        || er.vt_range_bwd.is_some()
        || er.ss_range_bwd.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_supersedes_a_same_file_window_commit() {
        // The focus-steal path collects [AutoFitAll, Window] for the SAME file in one
        // frame; the reset must win (the Window would re-pin what the reset clears).
        let id = "file-1".to_string();
        let mut commits = vec![
            Commit::AutoFitAll { id: id.clone() },
            Commit::Window {
                id,
                which: ExpertWindow::FwdVt,
                win: Some((0.5, 1.5)),
            },
        ];
        drop_windows_superseded_by_reset(&mut commits);
        assert_eq!(commits.len(), 1);
        assert!(matches!(commits[0], Commit::AutoFitAll { .. }));
    }

    #[test]
    fn a_window_commit_without_a_reset_is_kept() {
        let mut commits = vec![Commit::Window {
            id: "file-1".to_string(),
            which: ExpertWindow::FwdVt,
            win: Some((0.5, 1.5)),
        }];
        drop_windows_superseded_by_reset(&mut commits);
        assert_eq!(
            commits.len(),
            1,
            "a normal edit with no reset must still commit"
        );
    }

    #[test]
    fn reset_does_not_drop_a_different_files_window() {
        let mut commits = vec![
            Commit::AutoFitAll {
                id: "file-1".to_string(),
            },
            Commit::Window {
                id: "file-2".to_string(),
                which: ExpertWindow::FwdVt,
                win: Some((0.5, 1.5)),
            },
        ];
        drop_windows_superseded_by_reset(&mut commits);
        assert_eq!(
            commits.len(),
            2,
            "a reset on one file must not drop another file's window"
        );
    }
}
