//! Faithful re-implementations of the numpy/pandas semantics ParamEx relies on.
//!
//! Every function here is pinned by committed reference snapshots under
//! `tests/reference/numpy_compat/` (tests under `tests/shared/numpy_compat/`).
//! Do NOT replace these with algebraically-equivalent rewrites — the goldens
//! pin exact behaviour (tie-breaks, rounding, edge attenuation, NaN handling).

mod order;
mod scalar;
mod series;
mod stats;

pub use order::{argsort, searchsorted, take_by, unique_mean, Side};
pub use scalar::{banker_round, isclose, ptp};
pub use series::{gradient, interp, linspace};
pub use stats::{nanargmax, nanargmin, nanmedian, std_sample};
