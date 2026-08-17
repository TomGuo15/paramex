//! Compact-model fitting ("Model Fit") workspace. A thin egui view over the
//! reference-checked `paramex_core::modelfit` engine: load/seed devices, switch
//! among supported compact models, and show the parameter table, fit overlay, AC
//! plots, output curves, and Verilog-A export action.

mod ingest;
pub mod models;
pub mod page;
pub mod panels;
pub mod state;

pub use page::show;

use crate::io_tasks::IoQueue;
use state::{IngestIssues, ModelFitState};

/// Model Fit's complete runtime aggregate.
#[derive(Default)]
pub struct ModelFitWorkspace {
    pub(crate) state: ModelFitState,
    pub(crate) io: IoQueue<ingest::Msg>,
    pub(crate) issues: IngestIssues,
}

impl ModelFitWorkspace {
    pub fn from_state(state: ModelFitState) -> Self {
        Self {
            state,
            io: IoQueue::default(),
            issues: IngestIssues::default(),
        }
    }

    pub fn state(&self) -> &ModelFitState {
        &self.state
    }

    pub fn record_ingest_error(&mut self, name: String, message: String) {
        self.issues.record(name, message);
    }

    pub fn has_ingest_errors(&self) -> bool {
        self.issues.has_errors()
    }

    pub(crate) fn drain_ingest(
        &mut self,
        ctx: &eframe::egui::Context,
        toasts: &mut egui_notify::Toasts,
    ) {
        ingest::drain(ctx, self, toasts);
    }

    #[cfg(test)]
    pub(crate) fn is_busy(&self) -> bool {
        self.io.is_busy()
    }

    pub fn is_idle(&self) -> bool {
        self.io.is_idle()
    }
}
