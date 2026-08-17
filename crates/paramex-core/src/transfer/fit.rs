//! Transfer-facing windowed linear-regression adapter (`windowed_fitter.py`).
//!
//! A [`WindowedFitter`] is built once for a `(sweep, transform)` pair and then
//! answers many windowed `.fit`/`.fit_indices` queries in O(1) each, via prefix
//! sums precomputed by the product-neutral shared fitter. Callers apply their
//! own R² / min-points gating afterwards.

use crate::shared::numerics::{WindowedLinearFit, WindowedLinearFitter};
use crate::transfer::types::{SweepData, WindowedFitResult};

/// Which transform of `|Id|` is regressed against `Vg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    /// `sqrt(|Id|)` vs `Vg` — the V_TH ELR fit.
    Sqrt,
    /// `log10(|Id|)` vs `Vg` — the SS fit.
    Log,
}

/// Windowed linear-regression engine over a single transformed sweep.
pub struct WindowedFitter {
    inner: WindowedLinearFitter,
}

impl WindowedFitter {
    /// Build the fitter (`windowed_fitter.py:39-67`).
    ///
    /// Samples are kept where `vg` and `id_abs` are finite and `id_abs > 0`;
    /// `y` is `sqrt(|Id|)` or `log10(|Id|)` per `transform`; the kept samples are
    /// sorted ascending by `vg`. Prefix sums of `x`, `y`, `x²`, `y²`, `xy` are
    /// then precomputed.
    ///
    /// NOTE (parity): this mask, the transform, and the sort are relied on by the
    /// VT window selector to be byte-identical to its own candidate preparation
    /// (guarded by the lifted `test_vt_window_equivalence`). The sort here is a
    /// stable sort by `total_cmp`, which matches numpy's `argsort` on the
    /// unique-Vg data production and the corpus use; exotic duplicate-Vg
    /// tie-order parity is covered by the equivalence tests. Fit *results* are unaffected by
    /// tie order (the prefix sums commute within any window).
    pub fn new(sweep: &SweepData, transform: Transform) -> Self {
        let samples = sweep
            .vg
            .iter()
            .copied()
            .zip(sweep.id_abs.iter().copied())
            .filter(|(voltage, current)| {
                voltage.is_finite() && current.is_finite() && *current > 0.0
            })
            .map(|(voltage, current)| {
                let current = current.abs();
                let value = match transform {
                    Transform::Sqrt => current.sqrt(),
                    Transform::Log => current.log10(),
                };
                (voltage, value)
            });
        Self {
            inner: WindowedLinearFitter::new(samples),
        }
    }

    /// The finite, positive, Vg-sorted sample abscissae (read-only).
    pub fn x(&self) -> &[f64] {
        self.inner.x()
    }

    /// Number of usable samples after masking.
    pub fn n(&self) -> usize {
        self.inner.len()
    }

    /// Linear fit over the half-open positional window `[start, end)`
    /// (`windowed_fitter.py:94-102`). O(1) via the prefix sums. `start`/`end`
    /// index into [`Self::x`]. Use this for window *searches*. Public callers
    /// may pass stale/out-of-range indices; they are clamped to the available
    /// samples so malformed input yields a NaN fit instead of panicking.
    pub fn fit_indices(&self, start: usize, end: usize) -> WindowedFitResult {
        into_transfer_result(self.inner.fit_indices(start, end))
    }

    /// Fit the samples whose Vg lies in `fit_range` (`windowed_fitter.py:79-92`).
    ///
    /// `None` means "use every sample." Bounds are inclusive on both ends; they
    /// are sorted ascending first. The window is located with
    /// `searchsorted(x, lo, "left")` .. `searchsorted(x, hi, "right")`, then fed
    /// to [`Self::fit_indices`].
    pub fn fit(&self, fit_range: Option<(f64, f64)>) -> WindowedFitResult {
        into_transfer_result(self.inner.fit(fit_range))
    }
}

fn into_transfer_result(fit: WindowedLinearFit) -> WindowedFitResult {
    WindowedFitResult {
        slope: fit.slope,
        intercept: fit.intercept,
        r2: fit.r2,
        points: fit.points,
    }
}
