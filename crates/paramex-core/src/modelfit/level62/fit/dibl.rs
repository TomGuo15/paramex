use super::super::forward::level62_current;
use super::super::params::Level62Params;
use crate::modelfit::extract::{prepare_transfer, PreparedTransfer};
use crate::modelfit::optimize::{levenberg_marquardt, LevMarOptions};
use crate::modelfit::types::{GeometryParams, Polarity};

struct PreparedDiblSweep {
    transfer: PreparedTransfer,
    vds_normalized: f64,
}

/// Refine the Level 62 **DIBL** strength from transfers at two (or more) distinct
/// drain biases: a joint Levenberg–Marquardt fit of `[VTO, log10(AT)]`
/// over the concatenated `log10(Id)` overlays, each sweep evaluated at its own
/// `V_DS`. Everything else stays fixed — the base extraction folded the primary
/// bias's DIBL shift into its `VTO`, so `VTO` must be co-freed, while `BT` (degenerate
/// with `VTO` at one geometry) and `VSI`/`VST` (second-order shape) stay put.
///
/// `transfers` are `(device-frame Vg, |Id|, device-frame V_DS)` sweeps; polarity is
/// folded internally (a sweep whose detected polarity disagrees is skipped). Returns
/// `None` when fewer than two usable sweeps at meaningfully different `|V_DS|`
/// remain, or when no start beats the incoming parameters on the joint objective.
pub(in crate::modelfit) fn refine_level62_dibl(
    params: Level62Params,
    transfers: &[(&[f64], &[f64], f64)],
    geom: GeometryParams,
    temp_k: f64,
    polarity: Polarity,
) -> Option<Level62Params> {
    const SWEEP_POINT_CAP: usize = 200;
    let s = polarity.sign();
    // Each normalized transfer stays coupled to its polarity while the sweep is
    // capped and paired with its normalized drain bias.
    let mut sweeps: Vec<PreparedDiblSweep> = Vec::new();
    for &(vg, id, vds_dev) in transfers {
        let vds_n = s * vds_dev;
        if vds_n <= 0.0 || !vds_n.is_finite() {
            continue;
        }
        let Some(prepared) = prepare_transfer(vg, id) else {
            continue;
        };
        if prepared.polarity() != polarity {
            continue;
        }
        let transfer = prepared.decimated(SWEEP_POINT_CAP);
        if transfer.vg().len() >= 10 {
            sweeps.push(PreparedDiblSweep {
                transfer,
                vds_normalized: vds_n,
            });
        }
    }
    if sweeps.len() < 2 {
        return None;
    }
    let vds_lo = sweeps
        .iter()
        .map(|sweep| sweep.vds_normalized)
        .fold(f64::INFINITY, f64::min);
    let vds_hi = sweeps
        .iter()
        .map(|sweep| sweep.vds_normalized)
        .fold(0.0, f64::max);
    // DIBL is unidentifiable from near-equal biases: require a real Vds lever.
    if vds_hi < 1.5 * vds_lo && (vds_hi - vds_lo) < 1.0 {
        return None;
    }

    let build = |x: &[f64]| Level62Params {
        vto: x[0],
        at: 10f64.powf(x[1]),
        ..params
    };
    let residual = |x: &[f64]| -> Vec<f64> {
        let p = build(x);
        sweeps
            .iter()
            .flat_map(|sweep| {
                sweep
                    .transfer
                    .vg()
                    .iter()
                    .zip(sweep.transfer.id())
                    .map(move |(&v, &i)| {
                        level62_current(&p, geom, temp_k, v, sweep.vds_normalized)
                            .max(1.0e-30)
                            .log10()
                            - i.max(1.0e-30).log10()
                    })
            })
            .collect()
    };
    let cost_of = |p: &Level62Params| -> f64 {
        sweeps
            .iter()
            .flat_map(|sweep| {
                sweep
                    .transfer
                    .vg()
                    .iter()
                    .zip(sweep.transfer.id())
                    .map(move |(&v, &i)| {
                        level62_current(p, geom, temp_k, v, sweep.vds_normalized)
                            .max(1.0e-30)
                            .log10()
                            - i.max(1.0e-30).log10()
                    })
            })
            .map(|r| r * r)
            .sum::<f64>()
    };

    let lo = [params.vto - 4.0, -12.0];
    let hi = [params.vto + 4.0, -4.0];
    let opts = LevMarOptions {
        max_iters: 300,
        ..LevMarOptions::default()
    };
    let mut best: Option<(f64, Vec<f64>)> = None;
    // Multi-start over AT decades (the manual default 3e-8 sits at −7.5).
    for at_log in [-9.0, -7.5, -6.0] {
        let x0 = [params.vto, at_log];
        let out = levenberg_marquardt(residual, &x0, Some((&lo, &hi)), &opts);
        if out.cost.is_finite() && best.as_ref().is_none_or(|(c, _)| out.cost < *c) {
            best = Some((out.cost, out.params));
        }
    }
    let (best_cost, x) = best?;
    // Accept only a genuine improvement over the incoming parameters on the SAME
    // joint objective (0.5·Σr² is the Model Fit optimizer's cost convention).
    if best_cost >= 0.5 * cost_of(&params) {
        return None;
    }
    // Honesty guard: AT at (or hugging) the search floor means the optimizer found
    // no Vds²-scaled threshold shift and "improved" the joint cost through VTO
    // alone — e.g. a low-Vds sweep that never reaches turn-on. Returning that would
    // mangle the primary fit's threshold while claiming a DIBL fit, so decline.
    if x[1] <= lo[1] + 1.0 {
        return None;
    }
    Some(build(&x))
}
