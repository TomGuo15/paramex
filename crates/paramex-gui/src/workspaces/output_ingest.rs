//! Accounting shared by Transfer and Model Fit output-ingest batches.

use crate::format_ui::{model_fit_output_summary, transfer_output_summary};

#[derive(Default)]
pub(crate) struct OutputIngestStats {
    pub(crate) attached: usize,
    pub(crate) unmatched: usize,
    pub(crate) ambiguous: usize,
    pub(crate) displaced: usize,
    pub(crate) unfittable: usize,
    pub(crate) errors: usize,
    first_err: Option<String>,
}

impl OutputIngestStats {
    pub(crate) fn record_error(&mut self, message: String) {
        self.errors += 1;
        self.first_err.get_or_insert(message);
    }

    pub(crate) fn transfer_summary(&self) -> String {
        transfer_output_summary(
            self.attached,
            self.unmatched,
            self.ambiguous,
            self.displaced,
            self.errors,
            self.first_err.as_deref(),
        )
    }

    pub(crate) fn model_fit_summary(&self) -> String {
        model_fit_output_summary(
            self.attached,
            self.unmatched,
            self.ambiguous,
            self.displaced,
            self.unfittable,
            self.errors,
            self.first_err.as_deref(),
        )
    }
}
