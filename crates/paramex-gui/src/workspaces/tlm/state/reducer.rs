//! TLM state reducers: load install, clear, and V_G recompute mutation rules.

use paramex_core::tlm::{analyze_dataset, analyze_sweep};

use super::{TlmAnalyzed, TlmRows, TlmState, TlmTab};

impl TlmState {
    /// Install a worker-analyzed load: select the engine-default V_G + first group.
    pub fn install_analyzed(&mut self, analyzed: TlmAnalyzed) {
        let (dataset, result, sweep) = analyzed.into_parts();
        self.selected_vg = Some(result.selected_vg);
        self.selected_group = result.first_group_name().map(str::to_owned);
        // Pre-format all four table row sets from the fresh analyses, once.
        self.rows = TlmRows::from_analyses(&result, &sweep);
        self.rows_generation += 1;
        self.result = Some(result);
        self.sweep = Some(sweep);
        self.dataset = Some(dataset);
        self.load_error = None;
        // A fresh load always lands on Results. Failed workbooks are surfaced by
        // the always-visible right-column FILES card, so no auto-switch is needed.
        self.results_tab = TlmTab::Results;
    }

    /// Drop the loaded dataset + analyses (committed settings, the fallback V_D,
    /// survive; only load-derived state is dropped).
    pub fn clear(&mut self) {
        self.dataset = None;
        self.result = None;
        self.sweep = None;
        self.selected_group = None;
        self.selected_vg = None;
        self.load_error = None;
        self.results_tab = TlmTab::Results;
        self.rows = TlmRows::default();
        self.rows_generation += 1;
    }

    /// Re-analyze the loaded dataset at `requested_vg`; the engine snaps it to the
    /// nearest measured V_G. No-op when nothing is loaded. The sweep is unaffected.
    pub fn recompute_at_vg(&mut self, requested_vg: f64) {
        if let Some(dataset) = self.dataset.as_ref() {
            let result = analyze_dataset(dataset, Some(requested_vg));
            self.selected_vg = Some(result.selected_vg);
            // Keep the current group if it still exists; else fall back to the first.
            if self
                .selected_group
                .as_ref()
                .map(|g| !result.has_group(g))
                .unwrap_or(true)
            {
                self.selected_group = result.first_group_name().map(str::to_owned);
            }
            self.rows.refresh_selected_vg(&result);
            self.rows_generation += 1;
            self.result = Some(result);
        }
    }

    /// Remove one loaded/errored file (by its FILES-table relative path) from the
    /// dataset and re-analyze the remainder, so a bad length-point outlier or a
    /// failed workbook can be dropped without reloading the folder. The user's V_G
    /// and group survive (V_G snaps to the nearest remaining value, the group falls
    /// back to the first if it disappears). Clears everything when the last curve
    /// goes. Returns the number of status rows removed, including residual failure
    /// rows cleared with the final valid curve; zero means nothing matched.
    pub fn remove_file(&mut self, file: &str) -> usize {
        let Some(dataset) = self.dataset.take() else {
            return 0;
        };
        let removal = dataset.remove_workbook(file);
        let removed = removal.removed_statuses;
        let Some(dataset) = removal.dataset else {
            self.clear();
            return removed;
        };
        self.dataset = Some(dataset);
        if removed == 0 {
            return 0;
        }
        let requested_vg = self.selected_vg;
        let dataset = self.dataset.as_ref().unwrap();
        let result = analyze_dataset(dataset, requested_vg);
        let sweep = analyze_sweep(dataset);
        self.selected_vg = Some(result.selected_vg);
        if self
            .selected_group
            .as_ref()
            .map(|g| !result.has_group(g))
            .unwrap_or(true)
        {
            self.selected_group = result.first_group_name().map(str::to_owned);
        }
        self.rows = TlmRows::from_analyses(&result, &sweep);
        self.rows_generation += 1;
        self.result = Some(result);
        self.sweep = Some(sweep);
        removed
    }

    /// Select an analyzed process group by name. Unknown groups are ignored.
    pub fn select_group(&mut self, name: &str) -> bool {
        let Some(result) = self.result.as_ref() else {
            return false;
        };
        if !result.has_group(name) {
            return false;
        }
        self.selected_group = Some(name.to_owned());
        true
    }
}
