//! TLM-page state: the loaded `TlmDataset` + its analyses, the selected group + V_G,
//! the committed fallback V_D, and the persistent load-error row. All science comes
//! from `paramex_core::tlm`; load-time analysis runs on the worker thread and arrives
//! as a [`TlmAnalyzed`] bundle.

mod reducer;
mod rows;

use paramex_core::tlm::{
    analyze_dataset, analyze_sweep, result_csv, sweep_csv, valid_vd, GroupAnalysis,
    TlmAnalysisResult, TlmDataset, TlmParseError, TlmSweepResult,
};

use rows::error_count;
pub use rows::TlmRows;

/// Which table the TLM results card shows. Transient view state. (File statuses
/// are not a tab: they have their own always-visible right-column FILES card.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TlmTab {
    #[default]
    Results,
    Sweep,
    Lengths,
}

impl TlmTab {
    pub fn index(self) -> usize {
        match self {
            TlmTab::Results => 0,
            TlmTab::Sweep => 1,
            TlmTab::Lengths => 2,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => TlmTab::Sweep,
            2 => TlmTab::Lengths,
            _ => TlmTab::Results,
        }
    }
}

/// A fully analyzed load: computed on the WORKER thread so the UI thread only
/// installs the results (no analysis cost in the frame that drains the message).
pub struct TlmAnalyzed {
    dataset: TlmDataset,
    result: TlmAnalysisResult,
    sweep: TlmSweepResult,
}
impl TlmAnalyzed {
    /// Analyze one dataset into the coherent load bundle installed by the GUI.
    pub fn analyze(dataset: TlmDataset) -> Self {
        let result = analyze_dataset(&dataset, None);
        let sweep = analyze_sweep(&dataset);
        Self {
            dataset,
            result,
            sweep,
        }
    }

    pub fn workbook_count(&self) -> usize {
        self.dataset.statuses().len()
    }

    pub fn group_count(&self) -> usize {
        self.result.groups.len()
    }

    fn into_parts(self) -> (TlmDataset, TlmAnalysisResult, TlmSweepResult) {
        (self.dataset, self.result, self.sweep)
    }
}

/// Folder/count projection for the DATA card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TlmFolderSummary<'a> {
    pub root: &'a str,
    pub workbooks: usize,
    pub groups: usize,
}

/// Render-ready projection for the DATA card.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TlmDataCard<'a> {
    pub folder: Option<TlmFolderSummary<'a>>,
    pub load_error: Option<&'a str>,
    pub fallback_vd: f64,
    pub has_dataset: bool,
}

/// Render-ready projection for the TLM GROUPS card.
pub struct TlmGroupList<'a> {
    pub groups: &'a [GroupAnalysis],
    pub selected: Option<&'a str>,
}

/// Render-ready projection for the TLM ANALYSIS V_G picker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TlmVgPicker<'a> {
    pub vg_values: &'a [f64],
    pub selected_vg: f64,
}

/// Render-ready projection for the RESULTS card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlmResultsCard {
    pub active_tab: TlmTab,
    pub has_result: bool,
    pub has_sweep: bool,
}

/// Render-ready projection for the TLM FILES status card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TlmFilesCard {
    pub status_count: usize,
    pub error_count: usize,
}

/// Transient TLM workspace state (not committed; never round-trips to CSV).
pub struct TlmState {
    /// The loaded dataset (kept so group/V_G changes re-analyze without re-reading disk).
    dataset: Option<TlmDataset>,
    /// Single-V_G analysis at `selected_vg`.
    result: Option<TlmAnalysisResult>,
    /// Full V_G sweep (independent of `selected_vg`).
    sweep: Option<TlmSweepResult>,
    /// Selected process group (a name in the current analysis result).
    selected_group: Option<String>,
    /// Selected gate voltage (a measured value in `result.vg_values`).
    selected_vg: Option<f64>,
    /// Committed fallback V_D (EditBuffers pattern; applies at the next load).
    fallback_vd: f64,
    /// A failed folder load, shown as a persistent dismissible error row.
    load_error: Option<String>,
    /// Which results table tab is active (transient view state).
    results_tab: TlmTab,
    /// Pre-formatted table rows for the current analyses. PRIVATE so only the
    /// reducers can rebuild them (each rebuild bumps `rows_generation`, which the
    /// render side keys its measurement cache on); render reads via [`Self::rows`]
    /// and never builds rows.
    rows: TlmRows,
    /// Bumped on every `rows` rebuild — the render-side measurement-cache key.
    rows_generation: u64,
}

impl Default for TlmState {
    fn default() -> Self {
        TlmState {
            dataset: None,
            result: None,
            sweep: None,
            selected_group: None,
            selected_vg: None,
            fallback_vd: -0.5,
            load_error: None,
            results_tab: TlmTab::Results,
            rows: TlmRows::default(),
            rows_generation: 0,
        }
    }
}

impl TlmState {
    /// The pre-formatted table rows for the current analyses (read-only — only
    /// the reducers rebuild them, in lockstep with [`Self::rows_generation`]).
    pub fn rows(&self) -> &TlmRows {
        &self.rows
    }

    /// Monotonic id of the current `rows` contents — the key the render side
    /// uses to invalidate its per-table grid measurements.
    pub fn rows_generation(&self) -> u64 {
        self.rows_generation
    }

    /// Render-ready DATA card state.
    pub fn data_card(&self) -> TlmDataCard<'_> {
        let folder = self
            .dataset
            .as_ref()
            .zip(self.result.as_ref())
            .map(|(dataset, result)| TlmFolderSummary {
                root: dataset.root(),
                workbooks: result.statuses.len(),
                groups: result.groups.len(),
            });
        TlmDataCard {
            folder,
            load_error: self.load_error.as_deref(),
            fallback_vd: self.fallback_vd,
            has_dataset: self.dataset.is_some(),
        }
    }

    pub fn has_dataset(&self) -> bool {
        self.dataset.is_some()
    }

    pub fn has_load_error(&self) -> bool {
        self.load_error.is_some()
    }

    pub fn fallback_vd(&self) -> f64 {
        self.fallback_vd
    }

    pub fn set_fallback_vd(&mut self, value: f64) -> Result<(), TlmParseError> {
        let value = valid_vd(value, "Fallback V_D")?;
        self.fallback_vd = value;
        Ok(())
    }

    pub fn set_load_error(&mut self, message: String) {
        self.load_error = Some(message);
    }

    pub fn dismiss_load_error(&mut self) {
        self.load_error = None;
    }

    /// Render-ready TLM GROUPS card state.
    pub fn group_list(&self) -> Option<TlmGroupList<'_>> {
        Some(TlmGroupList {
            groups: &self.result.as_ref()?.groups,
            selected: self.selected_group.as_deref(),
        })
    }

    pub fn selected_group_name(&self) -> Option<&str> {
        self.selected_group.as_deref()
    }

    /// Render-ready TLM ANALYSIS V_G picker state.
    pub fn vg_picker(&self) -> Option<TlmVgPicker<'_>> {
        let result = self.result.as_ref()?;
        Some(TlmVgPicker {
            vg_values: &result.vg_values,
            selected_vg: self.selected_vg.unwrap_or(result.selected_vg),
        })
    }

    pub fn selected_vg(&self) -> Option<f64> {
        self.selected_vg
    }

    /// Render-ready RESULTS card state.
    pub fn results_card(&self) -> TlmResultsCard {
        TlmResultsCard {
            active_tab: self.results_tab,
            has_result: self.result.is_some(),
            has_sweep: self.sweep.is_some(),
        }
    }

    pub fn results_tab(&self) -> TlmTab {
        self.results_tab
    }

    pub fn set_results_tab(&mut self, tab: TlmTab) {
        self.results_tab = tab;
    }

    pub fn result_csv_bytes(&self) -> Option<Vec<u8>> {
        self.result.as_ref().map(result_csv)
    }

    pub fn sweep_csv_bytes(&self) -> Option<Vec<u8>> {
        self.sweep.as_ref().map(sweep_csv)
    }

    /// Render-ready TLM FILES card state.
    pub fn files_card(&self) -> Option<TlmFilesCard> {
        let result = self.result.as_ref()?;
        Some(TlmFilesCard {
            status_count: result.statuses.len(),
            error_count: error_count(result),
        })
    }

    /// The `GroupAnalysis` for the selected group at the current V_G, if any.
    pub fn selected_group_analysis(&self) -> Option<&GroupAnalysis> {
        let result = self.result.as_ref()?;
        let name = self.selected_group.as_ref()?;
        result.group(name)
    }
}
