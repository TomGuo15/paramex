//! Public data types for the transfer-extraction pipeline: sweep/fit/metric
//! DTOs plus geometry/settings and Cox helper facades.

use std::path::PathBuf;

mod cox;
mod geometry;

pub(in crate::transfer) use cox::validate_cox_nf_per_cm2;
pub use cox::{calculate_stack_cox_nf_per_cm2, CoxError};
pub use geometry::{DeviceGeometry, ExtractionSettings};

/// A single transfer sweep: gate voltage `vg` paired index-for-index with the
/// absolute drain current `id_abs`. Mirrors `extraction.types.SweepData`.
#[derive(Debug, Clone, PartialEq)]
pub struct SweepData {
    pub vg: Vec<f64>,
    pub id_abs: Vec<f64>,
}

/// Plain windowed linear-regression result; no metric-specific fields. Returned
/// by [`crate::transfer::WindowedFitter`]. Higher-level metric functions wrap this
/// after applying their own R² / min-points gating. Mirrors
/// `extraction.types.WindowedFitResult`.
///
/// `slope`, `intercept`, and `r2` are `NaN` when the window has fewer than two
/// samples or the regression is degenerate. `points` is the sample count in the
/// window (which may be 0 or 1 in the NaN cases).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowedFitResult {
    pub slope: f64,
    pub intercept: f64,
    pub r2: f64,
    pub points: usize,
}

/// A normalised transfer curve loaded from a user file. Mirrors
/// `extraction.types.ParsedCurve` (`types.py:15-22`).
///
/// `vg` and `id_abs` are index-for-index paired, already masked to finite,
/// strictly-positive `|Id|` samples by the parser. `source_path` is `None` for
/// in-memory (uploaded-bytes) parses. Fields remain public as a transport value;
/// [`crate::transfer::Session`] revalidates the parser contract before
/// persistent admission.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCurve {
    pub name: String,
    pub vg: Vec<f64>,
    pub id_abs: Vec<f64>,
    pub source_path: Option<PathBuf>,
}

/// Physics constants required for V_TH / mobility extraction. Mirrors
/// `extraction.types.ExtractionContext` (`types.py:33-39`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::transfer) struct ExtractionContext {
    pub(in crate::transfer) cox_f_per_cm2: f64,
    pub(in crate::transfer) aspect_ratio: f64,
}

/// Per-sweep extraction output. Mirrors `extraction.types.SweepExtractionResult`
/// (`types.py:41-50`). Float fields are `NaN` when their metric could not be
/// extracted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::transfer) struct SweepExtractionResult {
    pub(in crate::transfer) vt: f64,
    pub(in crate::transfer) mobility: f64,
    pub(in crate::transfer) ss_mv_dec: f64,
    pub(in crate::transfer) ion: f64,
    pub(in crate::transfer) ioff: f64,
    pub(in crate::transfer) on_off_ratio: f64,
}

/// Optional user-pinned extraction windows (`models.py:15-22` `ExpertRanges`).
/// Each is `None` when the user has not pinned that window (auto-select runs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ExpertRanges {
    pub vt_range: Option<(f64, f64)>,
    pub ss_range: Option<(f64, f64)>,
    pub vt_range_bwd: Option<(f64, f64)>,
    pub ss_range_bwd: Option<(f64, f64)>,
}

/// One display/export row for a parsed transfer curve (`models.py:58-92`
/// `MetricResult`, frozen). Float fields are `NaN` when their metric could not
/// be extracted; window fields are `None` when not selected.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricResult {
    pub filename: String,
    pub width_um: f64,
    pub length_um: f64,
    pub aspect_ratio: f64,
    pub geometry_source: String,
    pub vt: f64,
    pub mu_sat: f64,
    pub ss_mv_dec: f64,
    pub ion: f64,
    pub ioff: f64,
    pub on_off_ratio: f64,
    pub delta_vth_hysteresis: f64,
    pub vt_window: Option<(f64, f64)>,
    pub ss_window: Option<(f64, f64)>,
    pub vt_window_bwd: Option<(f64, f64)>,
    pub ss_window_bwd: Option<(f64, f64)>,
    pub status: String,
    pub message: String,
    pub has_backward_sweep: bool,
    pub vt_forward: f64,
    pub mu_sat_forward: f64,
    pub ss_mv_dec_forward: f64,
    pub ion_forward: f64,
    pub ioff_forward: f64,
    pub on_off_ratio_forward: f64,
    pub vt_backward: f64,
    pub mu_sat_backward: f64,
    pub ss_mv_dec_backward: f64,
    pub ion_backward: f64,
    pub ioff_backward: f64,
    pub on_off_ratio_backward: f64,
}
