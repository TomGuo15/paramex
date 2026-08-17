//! Series generation, interpolation, and derivative helpers.

/// Evenly spaced samples over the closed interval `[lo, hi]`, matching
/// `numpy.linspace(lo, hi, n)` (with the default `endpoint=True`).
///
/// Semantics (numpy is ground truth):
/// - `n == 0` -> `[]`.
/// - `n == 1` -> `[lo]` (`hi` is ignored, exactly as numpy).
/// - `n >= 2` -> `step = (hi - lo) / (n - 1)`, `point[i] = lo + i * step`,
///   and the LAST point is forced to be exactly `hi` to avoid endpoint
///   drift from floating-point accumulation.
///
/// All arithmetic is f64. Works for ascending, descending (`lo > hi`),
/// negative, and degenerate (`lo == hi`) ranges.
pub fn linspace(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![lo];
    }
    let mut out = Vec::with_capacity(n);
    let step = (hi - lo) / ((n - 1) as f64);
    for i in 0..n {
        out.push(lo + (i as f64) * step);
    }
    // Force the endpoint exactly to hi (numpy does this too).
    out[n - 1] = hi;
    out
}

/// Piecewise-linear interpolation matching `numpy.interp`.
///
/// Evaluates the linear interpolant of the data points `(xp[i], fp[i])` at each
/// query point in `xq`. `xp` is assumed strictly ascending (ties are tolerated
/// and resolved exactly as numpy does; see below). `xp` and `fp` must have the
/// same length.
///
/// Semantics (faithful to `np.interp`, which performs NO extrapolation):
/// - For `x < xp[0]`           -> clamps to `fp[0]`.
/// - For `x > xp[xp.len()-1]`  -> clamps to `fp[xp.len()-1]`.
/// - For `x` exactly on a knot -> returns that knot's `fp` value.
/// - Single knot (`xp.len() == 1`): every query returns `fp[0]`.
/// - Empty `xq`: returns an empty `Vec`.
///
/// Tie handling: when consecutive knots share an x value (a vertical step),
/// `xp` is non-decreasing rather than strictly increasing. numpy locates the
/// segment with a `searchsorted`-style right-biased binary search, so a query
/// landing on the duplicated x picks the LEFT (lower-index) knot's value. This
/// implementation reproduces that by using `partition_point` (equivalent to
/// `searchsorted(side="right")`) and clamping the resulting index, then
/// returning the left endpoint's value when the located segment has zero width.
///
/// Non-finite values (`NaN`/`inf`) in `fp` propagate through arithmetic exactly
/// as in numpy; out-of-range clamping simply returns the (possibly non-finite)
/// endpoint value.
///
/// # Panics
/// Panics if `xp.is_empty()` while `xq` is non-empty, or if `xp.len() != fp.len()`,
/// mirroring numpy raising `ValueError` for these malformed inputs.
pub fn interp(xq: &[f64], xp: &[f64], fp: &[f64]) -> Vec<f64> {
    assert_eq!(
        xp.len(),
        fp.len(),
        "interp: xp and fp must have equal length (got {} and {})",
        xp.len(),
        fp.len()
    );

    if xq.is_empty() {
        return Vec::new();
    }

    assert!(
        !xp.is_empty(),
        "interp: xp/fp must be non-empty when xq is non-empty"
    );

    let n = xp.len();
    let lo_x = xp[0];
    let hi_x = xp[n - 1];
    let lo_f = fp[0];
    let hi_f = fp[n - 1];

    let mut out = Vec::with_capacity(xq.len());

    for &x in xq {
        // numpy returns NaN for a NaN query: a NaN satisfies neither clamp
        // comparison below, which would otherwise fall through to an interior
        // index underflow. Guard it explicitly.
        if x.is_nan() {
            out.push(f64::NAN);
            continue;
        }
        // Handle the well-defined clamp branches first.
        if x <= lo_x {
            // Covers x < xp[0] (clamp) and x == xp[0] (on-knot) -> fp[0].
            out.push(lo_f);
            continue;
        }
        if x >= hi_x {
            // Covers x > xp[-1] (clamp) and x == xp[-1] (on-knot) -> fp[-1].
            out.push(hi_f);
            continue;
        }

        // Interior: lo_x < x < hi_x, and n >= 2 here (single-knot collapses to
        // the two clamp branches above since lo_x == hi_x).
        // Right-biased search == numpy/searchsorted(side="right"): index of the
        // first knot strictly greater than x.
        let hi = xp.partition_point(|&knot| knot <= x);
        // hi is in 1..=n-1 because lo_x < x < hi_x.
        let lo = hi - 1;

        let x0 = xp[lo];
        let x1 = xp[hi];
        let f0 = fp[lo];
        let f1 = fp[hi];

        let dx = x1 - x0;
        if dx == 0.0 {
            // Vertical step (tie): numpy returns the left endpoint value.
            out.push(f0);
        } else {
            let t = (x - x0) / dx;
            if t == 0.0 {
                // x is exactly on the left knot; avoid 0.0*(f1-f0) which is
                // NaN when f1 is infinite. Return f0 directly (numpy does the same).
                out.push(f0);
            } else {
                // f0 + t*(f1 - f0); matches numpy's slope*(x-x0)+f0 formulation
                // closely enough for the pinned 1e-12 tolerance.
                out.push(f0 + t * (f1 - f0));
            }
        }
    }

    out
}

/// Numerical gradient matching `numpy.gradient(y, x)` with the default
/// `edge_order=1`, for an arbitrary (possibly non-uniform) coordinate `x`.
///
/// Mirrors numpy exactly:
/// * Interior points use the 2nd-order accurate central difference for
///   unequal spacing. With left spacing `hs = x[i] - x[i-1]` and right
///   spacing `hd = x[i+1] - x[i]`:
///     out[i] = (-hd/(hs*(hs+hd)))      * y[i-1]
///            + ((hd - hs)/(hs*hd))     * y[i]
///            + ( hs/(hd*(hs+hd)))      * y[i+1]
///   (algebraically identical to numpy's coefficient form, which weights
///   the central node by `(dx2 - dx1)/(dx1*dx2)`).
/// * The first and last points use 1st-order one-sided differences:
///     out[0]   = (y[1] - y[0]) / (x[1] - x[0])
///     out[n-1] = (y[n-1] - y[n-2]) / (x[n-1] - x[n-2])
/// * For `len == 2` both points are endpoints, so both reduce to the same
///   one-sided difference.
///
/// # Panics
/// Panics if `y.len() < 2` or if `x.len() != y.len()`, matching the domain
/// of the numpy call we are porting (`np.gradient` requires len >= 2 for the
/// default `edge_order=1`, and a coordinate array sized to the data).
///
/// Non-finite inputs (NaN/inf) propagate through the arithmetic exactly as
/// they do in numpy/IEEE-754, so they are not special-cased.
#[allow(clippy::doc_overindented_list_items)] // formula alignment: indented for visual alignment, not markdown nesting
pub fn gradient(y: &[f64], x: &[f64]) -> Vec<f64> {
    let n = y.len();
    assert!(n >= 2, "gradient requires len >= 2, got {n}");
    assert!(
        x.len() == n,
        "gradient requires x.len() == y.len(), got x.len()={} y.len()={}",
        x.len(),
        n
    );

    let mut out = vec![0.0_f64; n];

    // First-order one-sided endpoints.
    out[0] = (y[1] - y[0]) / (x[1] - x[0]);
    out[n - 1] = (y[n - 1] - y[n - 2]) / (x[n - 1] - x[n - 2]);

    // Interior: 2nd-order central difference for unequal spacing.
    for i in 1..n - 1 {
        let hs = x[i] - x[i - 1]; // left spacing
        let hd = x[i + 1] - x[i]; // right spacing
        let a = -hd / (hs * (hs + hd));
        let b = (hd - hs) / (hs * hd);
        let c = hs / (hd * (hs + hd));
        out[i] = a * y[i - 1] + b * y[i] + c * y[i + 1];
    }

    out
}
