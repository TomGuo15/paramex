pub(crate) mod ingest;
pub mod page;
pub mod panels;
pub mod selector;
pub mod state;

use paramex_core::transfer::{AttachOutputOutcome, OutputDataset, ParsedCurve, Session};

use crate::io_tasks::IoQueue;
use panels::results_table::{OutputResultsTableCache, ResultsTableCache};
use state::{
    same_output_source, upsert_pending_output, FileRows, PendingOutput, PendingOutputReason,
    PlotCache, TransferUiState,
};

pub use page::show;

/// Transfer's complete runtime aggregate. Product state and display caches are
/// composed at the workspace seam so the state module does not depend on panel
/// implementation types.
pub struct TransferWorkspace {
    pub(crate) session: Session,
    pub(crate) pending_outputs: Vec<PendingOutput>,
    pub(crate) ui: TransferUiState,
    pub(crate) plot: PlotCache,
    pub(crate) results_cache: ResultsTableCache,
    pub(crate) output_results_cache: OutputResultsTableCache,
    pub(crate) io: IoQueue<ingest::Msg>,
    pub(crate) file_rows: FileRows,
}

impl TransferWorkspace {
    pub fn from_session(session: Session) -> Self {
        let mut file_rows = FileRows::default();
        for id in session.file_ids() {
            file_rows.record_file(id.to_owned());
        }
        Self {
            session,
            pending_outputs: Vec::new(),
            ui: TransferUiState::default(),
            plot: PlotCache::default(),
            results_cache: ResultsTableCache::default(),
            output_results_cache: OutputResultsTableCache::default(),
            io: IoQueue::default(),
            file_rows,
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn add_curve(&mut self, curve: ParsedCurve) -> Option<String> {
        let id = self.session.add_curve(curve)?;
        self.file_rows.record_file(id.clone());
        Some(id)
    }

    pub fn attach_output(&mut self, output: OutputDataset) -> AttachOutputOutcome {
        self.session.attach_output(output)
    }

    pub fn select_file(&mut self, file_id: &str) -> bool {
        self.session.select_file(file_id)
    }

    pub fn set_file_checked(&mut self, file_id: &str, checked: bool) -> bool {
        self.session.set_file_checked(file_id, checked)
    }

    pub fn remove_selected_or_checked(&mut self) -> usize {
        let removed = self.session.remove_selected_or_checked();
        if removed > 0 {
            self.prune_file_rows();
        }
        removed
    }

    pub fn keep_checked_files(&mut self) -> Option<usize> {
        let removed = self.session.keep_checked_files()?;
        if removed > 0 {
            self.prune_file_rows();
        }
        Some(removed)
    }

    pub fn clear_files(&mut self) -> usize {
        let removed = self.session.clear_files();
        if removed > 0 {
            self.prune_file_rows();
        }
        removed
    }

    pub fn results_view(&self) -> state::TransferResultsView {
        self.ui.results_view()
    }

    pub fn set_results_view(&mut self, view: state::TransferResultsView) {
        self.ui.set_results_view(view);
    }

    pub fn pending_outputs(&self) -> &[PendingOutput] {
        &self.pending_outputs
    }

    pub fn record_ingest_error(&mut self, name: impl Into<String>, message: impl Into<String>) {
        self.file_rows.record_error(name.into(), message.into());
    }

    pub fn has_ingest_errors(&self) -> bool {
        self.file_rows.has_errors()
    }

    pub fn record_pending_output(&mut self, dataset: OutputDataset, reason: PendingOutputReason) {
        // A folder re-scan re-parses the same unmatched output file every run;
        // replace the stale row instead of stacking duplicates.
        upsert_pending_output(&mut self.pending_outputs, dataset, reason);
    }

    pub(crate) fn retain_detached_output(&mut self, dataset: OutputDataset) {
        state::retain_detached_output(&mut self.pending_outputs, dataset);
    }

    pub(crate) fn drain_ingest(&mut self, toasts: &mut egui_notify::Toasts) {
        ingest::drain(self, toasts);
    }

    #[cfg(test)]
    pub(crate) fn is_busy(&self) -> bool {
        self.io.is_busy()
    }

    #[cfg(test)]
    pub(crate) fn is_idle(&self) -> bool {
        self.io.is_idle()
    }

    /// Drop any earlier pending classification for `dataset`'s source before
    /// an automatic attachment attempt. Unattached outcomes return the owned
    /// dataset and record its current classification again; a successful
    /// attachment therefore cannot leave a stale pending row behind.
    pub(crate) fn clear_pending_output_for(&mut self, dataset: &OutputDataset) {
        self.pending_outputs
            .retain(|pending| !same_output_source(pending.dataset(), dataset));
    }

    fn prune_file_rows(&mut self) {
        let session = &self.session;
        self.file_rows
            .prune_file_rows(|file_id| session.has_file(file_id));
    }
}

const TALL_PLOT_PAIR_ASPECT: f32 = 1.5;

pub(crate) fn plot_pair_should_stack(size: eframe::egui::Vec2) -> bool {
    size.x > 0.0 && size.y >= size.x * TALL_PLOT_PAIR_ASPECT
}

pub(crate) fn show_stacked_plot_pair(
    ui: &mut eframe::egui::Ui,
    id_salt: &'static str,
    height: f32,
    mut add: impl FnMut(&mut eframe::egui::Ui, usize),
) {
    let height = height.clamp(0.0, ui.available_height().max(0.0));
    let size = eframe::egui::vec2(ui.available_width().max(0.0), height);
    let (host, _) = ui.allocate_exact_size(size, eframe::egui::Sense::hover());
    let slot_h = ((host.height() - crate::layout::CARD_GAP).max(0.0) * 0.5).floor();

    for index in 0..2 {
        let top = host.top() + index as f32 * (slot_h + crate::layout::CARD_GAP);
        let bottom = if index == 1 {
            host.bottom()
        } else {
            top + slot_h
        };
        let rect = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(host.left(), top),
            eframe::egui::pos2(host.right(), bottom),
        );
        let mut child = ui.new_child(
            eframe::egui::UiBuilder::new()
                .id_salt((id_salt, index))
                .max_rect(rect)
                .layout(*ui.layout()),
        );
        child.set_min_size(rect.size());
        child.set_clip_rect(rect);
        add(&mut child, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve(name: &str) -> ParsedCurve {
        ParsedCurve {
            name: name.to_owned(),
            vg: (0..12).map(f64::from).collect(),
            id_abs: (1..=12).map(|index| index as f64 * 1.0e-9).collect(),
            source_path: None,
        }
    }

    fn output(path: Option<&str>, marker: f64) -> paramex_core::transfer::OutputDataset {
        paramex_core::transfer::OutputDataset {
            name: "shared_output.csv".to_owned(),
            curves: vec![paramex_core::transfer::OutputCurve {
                vg: marker,
                vd: vec![0.0, 1.0],
                id: vec![0.0, marker * 1.0e-6],
            }],
            source_path: path.map(Into::into),
        }
    }

    #[test]
    fn plot_pair_breakpoint_only_stacks_tall_narrow_bodies() {
        assert!(!plot_pair_should_stack(eframe::egui::vec2(638.0, 400.0)));
        assert!(!plot_pair_should_stack(eframe::egui::vec2(1100.0, 800.0)));
        assert!(plot_pair_should_stack(eframe::egui::vec2(638.0, 1075.0)));
    }

    #[test]
    fn workspace_curve_commands_keep_file_rows_in_sync() {
        let mut workspace = TransferWorkspace::from_session(Session::new());
        let id = workspace.add_curve(curve("device.csv")).expect("added");

        assert!(matches!(
            workspace.file_rows.rows().next(),
            Some(state::FileRow::File { id: row_id }) if row_id == id
        ));

        assert_eq!(workspace.remove_selected_or_checked(), 1);
        assert!(workspace.file_rows.rows().next().is_none());

        let kept = workspace.add_curve(curve("kept.csv")).expect("added");
        workspace.add_curve(curve("removed.csv")).expect("added");
        assert!(workspace.set_file_checked(&kept, true));
        assert_eq!(workspace.keep_checked_files(), Some(1));
        assert_eq!(
            workspace.file_rows.rows().collect::<Vec<_>>(),
            [state::FileRow::File { id: &kept }]
        );

        workspace.add_curve(curve("last.csv")).expect("added");
        assert_eq!(workspace.clear_files(), 2);
        assert!(workspace.file_rows.rows().next().is_none());
    }

    #[test]
    fn pending_output_upsert_folds_the_latest_sources_complete_match_set() {
        let mut latest_pathless = TransferWorkspace::from_session(Session::new());
        latest_pathless.record_pending_output(
            output(Some("lot-a/shared_output.csv"), 1.0),
            PendingOutputReason::NoMatch,
        );
        latest_pathless.record_pending_output(
            output(Some("lot-b/shared_output.csv"), 2.0),
            PendingOutputReason::Ambiguous,
        );
        latest_pathless.record_pending_output(output(None, 3.0), PendingOutputReason::Detached);

        assert_eq!(latest_pathless.pending_outputs().len(), 1);
        assert_eq!(
            latest_pathless.pending_outputs()[0].dataset().source_path,
            None
        );
        assert_eq!(
            latest_pathless.pending_outputs()[0].dataset().curves[0].vg,
            3.0
        );

        let mut latest_pathful = TransferWorkspace::from_session(Session::new());
        latest_pathful.record_pending_output(output(None, 3.0), PendingOutputReason::Detached);
        latest_pathful.record_pending_output(
            output(Some("lot-a/shared_output.csv"), 1.0),
            PendingOutputReason::NoMatch,
        );
        latest_pathful.record_pending_output(
            output(Some("lot-b/shared_output.csv"), 2.0),
            PendingOutputReason::Ambiguous,
        );

        assert_eq!(latest_pathful.pending_outputs().len(), 2);
        assert_eq!(
            latest_pathful
                .pending_outputs()
                .iter()
                .map(|pending| pending.dataset().source_path.as_deref())
                .collect::<Vec<_>>(),
            vec![
                Some(std::path::Path::new("lot-a/shared_output.csv")),
                Some(std::path::Path::new("lot-b/shared_output.csv")),
            ]
        );
    }
}
