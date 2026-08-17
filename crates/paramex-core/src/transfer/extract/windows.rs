//! Transfer V_TH and SS auto-window selection policy.

use crate::transfer::metrics::ss::select_ss_window;
use crate::transfer::metrics::vth::{select_elr_vt_window, DEFAULT_VT_R2_LADDER};

/// Selector defaults (Python `select_elr_vt_window`/`select_ss_window`
/// signature defaults; `metrics_adapter` calls both with no overrides).
const VT_WINDOW_SIZE: usize = 30;
const VT_STEP: usize = 1;
const VT_SEL_MIN_POINTS: usize = 10;
const VT_SEL_MIN_R2: f64 = 0.99;
const SS_MAX_POINTS: usize = 30;
const SS_MIN_DECADES: f64 = 1.0;
const SS_SEL_MIN_POINTS: usize = 5;
const SS_SEL_MIN_R2: f64 = 0.9;
const SS_OFF_GUARD_DECADES: f64 = 0.3;

pub(super) fn auto_vt_window(vg: &[f64], id_abs: &[f64]) -> Option<(f64, f64)> {
    select_elr_vt_window(
        vg,
        id_abs,
        VT_WINDOW_SIZE,
        VT_STEP,
        VT_SEL_MIN_POINTS,
        VT_SEL_MIN_R2,
        &DEFAULT_VT_R2_LADDER,
    )
}

pub(super) fn auto_ss_window(vg: &[f64], id_abs: &[f64]) -> Option<(f64, f64)> {
    select_ss_window(
        vg,
        id_abs,
        SS_MAX_POINTS,
        SS_MIN_DECADES,
        SS_SEL_MIN_POINTS,
        SS_SEL_MIN_R2,
        SS_OFF_GUARD_DECADES,
    )
}
