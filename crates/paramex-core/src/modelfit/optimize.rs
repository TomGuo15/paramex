//! Private Levenberg–Marquardt nonlinear least-squares backend for Model Fit.
//!
//! Minimizes `½·Σ rᵢ(p)²` over the parameter vector `p`. The caller supplies only a
//! forward `residual(p) -> [model − measured]`; the Jacobian is taken numerically
//! (central differences). Each step solves the damped normal equations
//! `(JᵀJ + λ·diag(JᵀJ))·δ = −Jᵀr` with an in-house Cholesky solve — **no linear-algebra
//! dependency**, to keep the binary lean (the fits here are small, `n ≤ ~20`).

/// Tuning knobs for [`levenberg_marquardt`]. Defaults suit small fits.
#[derive(Debug, Clone, Copy)]
pub(super) struct LevMarOptions {
    /// Maximum outer iterations.
    pub(super) max_iters: usize,
    /// Converge when one accepted step's relative cost decrease is below this.
    pub(super) ftol: f64,
    /// Converge when the gradient `‖Jᵀr‖∞` falls below this.
    pub(super) gtol: f64,
    /// Initial Marquardt damping `λ`.
    pub(super) init_lambda: f64,
}

impl Default for LevMarOptions {
    fn default() -> Self {
        LevMarOptions {
            max_iters: 200,
            ftol: 1e-10,
            gtol: 1e-10,
            init_lambda: 1e-3,
        }
    }
}

/// Result of a fit: the (bounds-respecting) parameters, the final cost
/// `½·Σ rᵢ²`, the iterations used, and whether a convergence test was met.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FitOutcome {
    pub(super) params: Vec<f64>,
    pub(super) cost: f64,
    pub(super) iters: usize,
    pub(super) converged: bool,
}

/// Fit `p` to minimize `½·Σ residual(p)²`, starting from `x0`. `bounds`, if given,
/// is `(lower, upper)` per parameter (each the length of `x0`); the step is
/// projected (clamped) into the box. Pass a closed-form estimate as `x0` for robust
/// convergence.
pub(super) fn levenberg_marquardt(
    residual: impl Fn(&[f64]) -> Vec<f64>,
    x0: &[f64],
    bounds: Option<(&[f64], &[f64])>,
    opts: &LevMarOptions,
) -> FitOutcome {
    let n = x0.len();
    if let Some((lo, hi)) = bounds {
        debug_assert_eq!(lo.len(), n, "lower-bounds length must match x0");
        debug_assert_eq!(hi.len(), n, "upper-bounds length must match x0");
    }
    let mut p = clamp(x0.to_vec(), bounds);
    let mut r = residual(&p);
    let mut cost = 0.5 * dot(&r, &r);
    let mut lambda = opts.init_lambda.max(1e-12);
    let mut converged = false;
    let mut iters = 0;

    for it in 0..opts.max_iters {
        iters = it + 1;
        let jac = jacobian(&residual, &p, r.len());
        let g = jt_r(&jac, &r, n); // Jᵀr   (length n)
        let h = jt_j(&jac, n); // JᵀJ   (n×n, row-major)

        if inf_norm(&g) < opts.gtol {
            converged = true;
            break;
        }

        // Inner loop: grow λ until a damped step decreases the cost (or give up).
        let neg_g: Vec<f64> = g.iter().map(|v| -v).collect();
        let mut accepted = false;
        for _ in 0..40 {
            let mut a = h.clone();
            for i in 0..n {
                // Marquardt scaling: damp by a multiple of the diagonal (floored so
                // a zero-curvature direction still gets a gradient step).
                a[i * n + i] += lambda * h[i * n + i].max(1e-12);
            }
            let Some(delta) = solve_spd(&a, &neg_g, n) else {
                lambda *= 10.0;
                if lambda > 1e12 {
                    break;
                }
                continue;
            };
            let p_new = clamp(add(&p, &delta), bounds);
            let r_new = residual(&p_new);
            let cost_new = 0.5 * dot(&r_new, &r_new);
            if cost_new < cost {
                let rel = (cost - cost_new) / cost.max(f64::MIN_POSITIVE);
                p = p_new;
                r = r_new;
                cost = cost_new;
                lambda = (lambda * 0.5).max(1e-12);
                accepted = true;
                if rel < opts.ftol {
                    converged = true;
                }
                break;
            }
            lambda *= 10.0;
            if lambda > 1e12 {
                break;
            }
        }

        if converged || !accepted {
            break;
        }
    }

    FitOutcome {
        params: p,
        cost,
        iters,
        converged,
    }
}

/// Clamp `p` into `[lo, hi]` per element when bounds are given. Tolerant of a caller passing
/// an INVERTED (`lo > hi`) or non-finite bound: `f64::clamp` panics when `min > max` or either
/// is NaN, so normalize to `[min(lo,hi), max(lo,hi)]` and skip a non-finite pair. A bad bound
/// must never crash the optimizer (it is reachable from a degenerate seed — e.g. a negative
/// power-law γ seed building `lo=0 > hi<0` in the Level 61 fit).
fn clamp(mut p: Vec<f64>, bounds: Option<(&[f64], &[f64])>) -> Vec<f64> {
    if let Some((lo, hi)) = bounds {
        for i in 0..p.len() {
            let l = lo[i].min(hi[i]);
            let h = lo[i].max(hi[i]);
            if l.is_finite() && h.is_finite() {
                p[i] = p[i].clamp(l, h);
            }
        }
    }
    p
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn add(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

fn inf_norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

/// Central-difference Jacobian `J` (m×n, row-major): `J[i·n+k] = ∂rᵢ/∂pₖ`.
fn jacobian(residual: &impl Fn(&[f64]) -> Vec<f64>, p: &[f64], m: usize) -> Vec<f64> {
    let n = p.len();
    let mut j = vec![0.0; m * n];
    let mut pk = p.to_vec();
    for k in 0..n {
        let h = (p[k].abs().max(1.0)) * 1e-6;
        pk[k] = p[k] + h;
        let r_plus = residual(&pk);
        pk[k] = p[k] - h;
        let r_minus = residual(&pk);
        pk[k] = p[k];
        let inv = 1.0 / (2.0 * h);
        for i in 0..m {
            j[i * n + k] = (r_plus[i] - r_minus[i]) * inv;
        }
    }
    j
}

/// `Jᵀr` (length n).
fn jt_r(j: &[f64], r: &[f64], n: usize) -> Vec<f64> {
    let m = r.len();
    let mut g = vec![0.0; n];
    for i in 0..m {
        let ri = r[i];
        for k in 0..n {
            g[k] += j[i * n + k] * ri;
        }
    }
    g
}

/// `JᵀJ` (n×n, row-major, symmetric).
fn jt_j(j: &[f64], n: usize) -> Vec<f64> {
    let m = j.len() / n;
    let mut h = vec![0.0; n * n];
    for i in 0..m {
        for a in 0..n {
            let jia = j[i * n + a];
            for b in a..n {
                h[a * n + b] += jia * j[i * n + b];
            }
        }
    }
    for a in 0..n {
        for b in (a + 1)..n {
            h[b * n + a] = h[a * n + b];
        }
    }
    h
}

/// Solve `A·x = b` for symmetric positive-definite `A` (n×n, row-major) by
/// Cholesky. `None` when `A` is not positive-definite (caller raises `λ`).
fn solve_spd(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n]; // lower-triangular
    for i in 0..n {
        for jc in 0..=i {
            let mut sum = a[i * n + jc];
            for k in 0..jc {
                sum -= l[i * n + k] * l[jc * n + k];
            }
            if i == jc {
                // Diagonal must be positive for a positive-definite A; NaN or ≤ 0
                // means not PD → bail so the caller raises λ.
                if sum <= 0.0 || sum.is_nan() {
                    return None;
                }
                l[i * n + jc] = sum.sqrt();
            } else {
                l[i * n + jc] = sum / l[jc * n + jc];
            }
        }
    }
    // forward solve L·y = b
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= l[i * n + k] * y[k];
        }
        y[i] = sum / l[i * n + i];
    }
    // back solve Lᵀ·x = y
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for k in (i + 1)..n {
            sum -= l[k * n + i] * x[k];
        }
        x[i] = sum / l[i * n + i];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recover (a, k) of y = a·e^{−k·x} from synthetic data, starting from a rough
    /// guess — the canonical nonlinear-least-squares smoke test.
    #[test]
    fn recovers_exponential_decay_params() {
        let (a_true, k_true) = (2.5, 0.7);
        let xs: Vec<f64> = (0..=40).map(|i| i as f64 * 0.25).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| a_true * (-k_true * x).exp()).collect();
        let residual = |p: &[f64]| -> Vec<f64> {
            xs.iter()
                .zip(&ys)
                .map(|(&x, &y)| p[0] * (-p[1] * x).exp() - y)
                .collect()
        };
        let out = levenberg_marquardt(residual, &[1.0, 1.0], None, &LevMarOptions::default());
        assert!(out.converged, "should converge ({} iters)", out.iters);
        assert!((out.params[0] - a_true).abs() < 1e-4, "a={}", out.params[0]);
        assert!((out.params[1] - k_true).abs() < 1e-4, "k={}", out.params[1]);
        assert!(out.cost < 1e-12, "near-zero residual, cost={}", out.cost);
    }

    /// An INVERTED bound (`lo > hi`) — reachable from a degenerate seed — must not panic
    /// (`f64::clamp` panics when `min > max`); the optimizer normalizes the box instead.
    #[test]
    fn inverted_or_nonfinite_bounds_do_not_panic() {
        let residual = |p: &[f64]| -> Vec<f64> { vec![p[0] - 1.0, p[1] - 1.0] };
        // p[0] bound inverted (0.0 > -0.5); p[1] bound has a NaN end.
        let lo = [0.0, 0.2];
        let hi = [-0.5, f64::NAN];
        let out = levenberg_marquardt(
            residual,
            &[0.3, 0.3],
            Some((&lo, &hi)),
            &LevMarOptions::default(),
        );
        // No panic, finite result, and the inverted box was honored as [-0.5, 0.0].
        assert!(
            out.params.iter().all(|v| v.is_finite()),
            "finite params: {:?}",
            out.params
        );
        assert!(
            (-0.5..=0.0).contains(&out.params[0]),
            "p0 in normalized box: {}",
            out.params[0]
        );
    }

    /// A 3-parameter fit (quadratic) recovers its coefficients.
    #[test]
    fn recovers_three_parameters() {
        let truth = [1.5, -2.0, 0.5];
        let xs: Vec<f64> = (0..=30).map(|i| i as f64 * 0.2 - 3.0).collect();
        let model = |p: &[f64], x: f64| p[0] + p[1] * x + p[2] * x * x;
        let ys: Vec<f64> = xs.iter().map(|&x| model(&truth, x)).collect();
        let residual = |p: &[f64]| -> Vec<f64> {
            xs.iter().zip(&ys).map(|(&x, &y)| model(p, x) - y).collect()
        };
        let out = levenberg_marquardt(residual, &[0.0, 0.0, 0.0], None, &LevMarOptions::default());
        assert!(out.converged);
        for (got, want) in out.params.iter().zip(&truth) {
            assert!((got - want).abs() < 1e-6, "got={got} want={want}");
        }
    }

    /// Bounds are respected: a parameter pinned by an upper bound below its true
    /// value must come back exactly at the bound.
    #[test]
    fn respects_parameter_bounds() {
        let (a_true, k_true) = (2.5, 0.7);
        let xs: Vec<f64> = (0..=40).map(|i| i as f64 * 0.25).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| a_true * (-k_true * x).exp()).collect();
        let residual = |p: &[f64]| -> Vec<f64> {
            xs.iter()
                .zip(&ys)
                .map(|(&x, &y)| p[0] * (-p[1] * x).exp() - y)
                .collect()
        };
        // Cap `a` at 2.0 (below its true 2.5): the fit must not exceed it.
        let lo = [0.0, 0.0];
        let hi = [2.0, 5.0];
        let out = levenberg_marquardt(
            residual,
            &[1.0, 1.0],
            Some((&lo, &hi)),
            &LevMarOptions::default(),
        );
        assert!(
            out.params[0] <= 2.0 + 1e-9,
            "a respected bound: {}",
            out.params[0]
        );
        assert!(out.params[1] >= 0.0, "k respected bound: {}", out.params[1]);
    }

    /// A rank-deficient problem (non-identifiable parameters → singular JᵀJ) must
    /// terminate cleanly — the Marquardt damping regularizes the singular direction,
    /// so no panic, no infinite loop, and the data is still fit.
    #[test]
    fn rank_deficient_problem_terminates_and_fits() {
        // y = (a + b)·x: only the SUM a+b is identifiable; JᵀJ is singular along (1,-1).
        let xs: Vec<f64> = (1..=20).map(|i| i as f64).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| 3.0 * x).collect(); // true a+b = 3
        let residual = |p: &[f64]| -> Vec<f64> {
            xs.iter()
                .zip(&ys)
                .map(|(&x, &y)| (p[0] + p[1]) * x - y)
                .collect()
        };
        let opts = LevMarOptions::default();
        let out = levenberg_marquardt(residual, &[0.0, 0.0], None, &opts);
        assert!(out.iters <= opts.max_iters, "bounded work");
        assert!(
            (out.params[0] + out.params[1] - 3.0).abs() < 1e-3,
            "fits the identifiable sum a+b: {}",
            out.params[0] + out.params[1]
        );
        assert!(
            out.cost < 1e-9,
            "data fit despite non-identifiability, cost={}",
            out.cost
        );
    }

    /// Starting at the exact optimum returns it (near-zero gradient → immediate stop),
    /// and the parameters are unchanged.
    #[test]
    fn already_optimal_is_a_fixed_point() {
        let xs: Vec<f64> = (0..=20).map(|i| i as f64 * 0.5).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| 3.0 * x + 1.0).collect();
        let residual = |p: &[f64]| -> Vec<f64> {
            xs.iter()
                .zip(&ys)
                .map(|(&x, &y)| p[0] * x + p[1] - y)
                .collect()
        };
        let out = levenberg_marquardt(residual, &[3.0, 1.0], None, &LevMarOptions::default());
        assert!(out.converged);
        assert!((out.params[0] - 3.0).abs() < 1e-9 && (out.params[1] - 1.0).abs() < 1e-9);
    }
}
