//! Product-neutral metrics over paired voltage/current slices.
//!
//! Transfer owns its extraction policy and result types. This module owns only
//! the slice algorithms also used by Model Fit.

mod subthreshold;

#[cfg(test)]
pub(crate) use subthreshold::{
    auto_select_subthreshold_window, detect_noise_floor_log10, select_local_subthreshold_window,
};
pub(crate) use subthreshold::{fit_subthreshold, select_subthreshold_window};
pub use subthreshold::{fit_subthreshold_auto, SubthresholdFit};

/// Return `(Ion, Ioff, Ion/Ioff)` from an absolute-current slice.
///
/// Non-finite samples are dropped, then magnitudes are taken. `Ion` is the max
/// magnitude; `Ioff` is the min strictly-positive magnitude (NaN if none); the
/// ratio is `Ion/Ioff` when `Ioff` is finite and positive, else NaN.
pub fn on_off_ratio(id_abs: &[f64]) -> (f64, f64, f64) {
    let values: Vec<f64> = id_abs
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .map(f64::abs)
        .collect();
    if values.is_empty() {
        return (f64::NAN, f64::NAN, f64::NAN);
    }

    let ion = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let ioff = values
        .iter()
        .copied()
        .filter(|&value| value > 0.0)
        .fold(f64::INFINITY, f64::min);
    let ioff = if ioff.is_finite() { ioff } else { f64::NAN };
    let ratio = if ioff.is_finite() && ioff > 0.0 {
        ion / ioff
    } else {
        f64::NAN
    };
    (ion, ioff, ratio)
}
