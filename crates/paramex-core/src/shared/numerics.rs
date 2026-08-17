//! General-purpose numeric helpers shared by the metric modules - no knowledge
//! of V_TH, SS, mobility, or any physical metric. Mirrors `extraction.numerics`.
//!
//! `FLOAT_EPSILON` is the project's unified tolerance. `fit` owns the
//! polyfit-path linear-fit engine.

mod fit;
mod windowed_fit;

pub use super::numpy_compat::unique_mean as collapse_duplicate_x;
pub use fit::linear_fit_with_r2;
pub(crate) use windowed_fit::{WindowedLinearFit, WindowedLinearFitter};

/// The project's unified floating-point tolerance (`numerics.py:13`).
pub const FLOAT_EPSILON: f64 = 1e-12;

/// Median of a slice, sorting it in place (ascending, NaN-tolerant). `None` when
/// empty. Shared by the modelfit extractors that combine per-gate estimates robustly.
pub(crate) fn median(xs: &mut [f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = xs.len() / 2;
    Some(if xs.len().is_multiple_of(2) {
        0.5 * (xs[mid - 1] + xs[mid])
    } else {
        xs[mid]
    })
}
