//! Plot axis-range + window-clamp helpers (`gui/plotting.py`).
//!
//! Pure math the egui plotting layer needs (axis bounds for `√I`/`log|I|` views
//! and clamping fit windows to the data axis). Kept in `core` so it is
//! golden-tested against the Python oracle instead of re-derived in the GUI.

use crate::shared::numpy_compat::isclose;

/// `(min, max)` of the finite entries of `values`; `(-1.0, 1.0)` when none are
/// finite; a degenerate `min == max` expands to `(min - 1, max + 1)`
/// (`plotting.py:433-442` `_axis_bounds`).
pub fn axis_bounds(values: &[f64]) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut any = false;
    for &v in values {
        if v.is_finite() {
            any = true;
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
        }
    }
    if !any {
        return (-1.0, 1.0);
    }
    if lo == hi {
        return (lo - 1.0, hi + 1.0);
    }
    (lo, hi)
}

/// Plotly log-y axis range for `|I_D|` (`plotting.py:363-374`
/// `_log_current_axis_range`). Uses only finite positive currents; `[-15, -3]`
/// when none. `np.isclose` (numpy defaults `rtol=1e-5`, `atol=1e-8`) widens a
/// degenerate range by ±1, then both ends pad by `max((hi - lo) * 0.08, 0.25)`.
pub fn log_current_axis_range(values: &[f64]) -> [f64; 2] {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut any = false;
    for &v in values {
        if v.is_finite() && v > 0.0 {
            any = true;
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
        }
    }
    if !any {
        return [-15.0, -3.0];
    }
    let mut lo = min.log10();
    let mut hi = max.log10();
    if isclose(lo, hi, 1e-5, 1e-8) {
        lo -= 1.0;
        hi += 1.0;
    }
    let pad = ((hi - lo) * 0.08).max(0.25);
    [lo - pad, hi + pad]
}

/// Plotly secondary sqrt-axis range (`plotting.py:377-385`
/// `_sqrt_current_axis_range`): `[0, 1.08 * max(√|I|)]` over finite positive
/// currents, or `[-0.1, 1.0]` when there is no usable positive current.
pub fn sqrt_current_axis_range(values: &[f64]) -> [f64; 2] {
    let mut hi = f64::NEG_INFINITY;
    let mut any = false;
    for &v in values {
        if v.is_finite() && v > 0.0 {
            any = true;
            let s = v.abs().sqrt();
            if s > hi {
                hi = s;
            }
        }
    }
    if !any || !hi.is_finite() || hi <= 0.0 {
        return [-0.1, 1.0];
    }
    [0.0, 1.08 * hi]
}

/// Clamp a window to the data-axis bounds; `None` when nothing remains
/// (`plotting.py:445-458` `_clamp_window_to_axis`). The window is sorted first;
/// `lo = max(axis_lo, lo)`, `hi = min(axis_hi, hi)`, `lo >= hi → None`.
pub fn clamp_window_to_axis(window: Option<(f64, f64)>, axis: (f64, f64)) -> Option<(f64, f64)> {
    let (a, b) = window?;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let lo = axis.0.max(lo);
    let hi = axis.1.min(hi);
    if lo >= hi {
        return None;
    }
    Some((lo, hi))
}
