//! Scalar numpy compatibility helpers.

/// numpy-compatible `np.isclose(a, b, rtol, atol)` for scalar f64 with `equal_nan=False`.
///
/// Returns `|a - b| <= atol + rtol * |b|`.
///
/// The tolerance is **asymmetric**: it is keyed to the magnitude of `b`, so
/// `isclose(a, b, ..)` is not in general equal to `isclose(b, a, ..)`.
///
/// numpy defaults are `rtol = 1e-5`, `atol = 1e-8`; callers may pass custom
/// tolerances such as `rtol = 0.0`, `atol = 1e-12` (pure absolute tolerance).
///
/// Non-finite handling (matching numpy with `equal_nan=False`):
/// - if either `a` or `b` is NaN, returns `false` (even NaN vs NaN);
/// - `+inf` vs `+inf` and `-inf` vs `-inf` return `true`;
/// - any inf vs a non-equal value (opposite-sign inf, or a finite number)
///   returns `false`.
pub fn isclose(a: f64, b: f64, rtol: f64, atol: f64) -> bool {
    // equal_nan = false: any NaN operand is never close.
    if a.is_nan() || b.is_nan() {
        return false;
    }
    // Infinities: close iff they are the exact same (sign-matching) infinity.
    // The finite tolerance formula would otherwise yield `inf <= inf` (true) for
    // +inf vs finite, which is wrong, so handle infinities explicitly.
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    (a - b).abs() <= atol + rtol * b.abs()
}

/// Round half-to-even ("banker's rounding"), matching Python's built-in
/// `round(x)` and `np.round(x)` with `decimals=0`.
///
/// Ties (exactly `.5`) round to the nearest **even** integer:
/// `0.5 -> 0`, `1.5 -> 2`, `2.5 -> 2`, `3.5 -> 4`, `-0.5 -> -0`, `-2.5 -> -2`.
/// Non-half values round to nearest; already-integral values are unchanged.
/// `NaN` and `+/-inf` pass through unchanged (as numpy does).
///
/// NOTE: this is NOT `f64::round`, which rounds ties *away from zero*
/// (`2.5 -> 3`) and would diverge from numpy/Python. We use
/// `f64::round_ties_even` (stable since Rust 1.77), the exact IEEE-754
/// round-to-nearest-ties-to-even operation numpy relies on.
///
/// Callers that need an index/count apply the cast *after* rounding, e.g.
/// `banker_round(n as f64 * frac) as usize`, which matches
/// `int(round(n * frac))` in the Python source.
pub fn banker_round(x: f64) -> f64 {
    x.round_ties_even()
}

/// Peak-to-peak range: `max - min`, matching `numpy.ptp`.
///
/// Assumes all values are finite (the caller is responsible for filtering
/// non-finite values). An empty slice yields `f64::NAN`, mirroring how the
/// golden encodes the null result for `np.ptp` on a size-0 array.
///
/// # Examples
/// ```
/// use paramex_core::shared::numpy_compat::ptp;
/// assert_eq!(ptp(&[1.0, 5.0, 3.0]), 4.0);
/// assert_eq!(ptp(&[4.0, 4.0, 4.0]), 0.0);
/// assert!(ptp(&[]).is_nan());
/// ```
pub fn ptp(vals: &[f64]) -> f64 {
    if vals.is_empty() {
        return f64::NAN;
    }
    let mut min = vals[0];
    let mut max = vals[0];
    for &v in &vals[1..] {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    max - min
}
