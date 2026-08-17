use super::super::forward::{level62_current, level62_output};
use super::super::params::Level62Params;
use crate::modelfit::extract::{extract_output, output_tail_slope, output_tail_targets};
use crate::modelfit::optimize::{levenberg_marquardt, LevMarOptions};
use crate::modelfit::transfer_preservation_log_r2;
use crate::modelfit::types::{GeometryParams, OutputCurve, Polarity};

/// Whether at least two on-state output curves contain a measured high-Vd rise
/// large enough to identify the Level 62 impact-ionization kink.
fn measured_kink_evidence(
    curves: &[OutputCurve],
    params: &Level62Params,
    polarity: Polarity,
) -> bool {
    let s = polarity.sign();
    let mut supporting_curves = 0;
    for curve in curves {
        if polarity.map_vg(curve.vg) <= params.vto {
            continue;
        }
        let mut points: Vec<_> = curve
            .vds
            .iter()
            .zip(&curve.id)
            .filter_map(|(&vd, &id)| {
                let vd = s * vd;
                let id = id.abs();
                (vd > 0.0 && vd.is_finite() && id > 0.0 && id.is_finite()).then_some((vd, id))
            })
            .collect();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(vd_max) = points.last().map(|point| point.0) else {
            continue;
        };
        let tail: Vec<_> = points
            .iter()
            .filter(|point| point.0 >= 0.8 * vd_max)
            .map(|point| point.1)
            .collect();
        if tail.len() < 4 {
            continue;
        }
        let window = (tail.len() / 4).max(1);
        let start = tail[..window].iter().sum::<f64>() / window as f64;
        let end = tail[tail.len() - window..].iter().sum::<f64>() / window as f64;
        if end >= 1.02 * start {
            supporting_curves += 1;
            if supporting_curves >= 2 {
                return true;
            }
        }
    }
    false
}

/// Refine the Level 62 output-dependent terms from measured Id-Vd families while
/// keeping the transfer-fitted shape fixed. ASAT/LAMBDA are always fitted while
/// MU0/MU1 are co-scaled to preserve the saturation gain at `transfer_vds`;
/// VKINK is freed only when the measured high-Vd tails contain kink evidence.
/// A guarded second stage constrains clean saturation-tail slopes without
/// accepting excessive per-curve current error or transfer-overlay loss.
pub(in crate::modelfit) fn refine_level62_output(
    params: Level62Params,
    curves: &[OutputCurve],
    geom: GeometryParams,
    temp_k: f64,
    transfer_vds: f64,
    transfer: (&[f64], &[f64]),
    polarity: Polarity,
) -> Option<Level62Params> {
    if !(transfer_vds.is_finite() && transfer_vds > 0.0) {
        return None;
    }
    let s = polarity.sign();
    let mut samples = Vec::new();
    for curve in curves {
        let vgs = polarity.map_vg(curve.vg);
        if vgs <= params.vto {
            continue;
        }
        for (&vd, &id) in curve.vds.iter().zip(&curve.id) {
            let vds = s * vd;
            let id = id.abs();
            if vds > 0.0 && id > 0.0 && vgs.is_finite() && vds.is_finite() && id.is_finite() {
                samples.push((vgs, vds, id));
            }
        }
    }
    if samples.len() < 20 {
        return None;
    }
    // Cap the whole family before the multi-start LM for the same reason
    // `FIT_POINT_CAP` caps the transfer path: the optimizer is O(samples) per
    // iteration and runs on the guarded Model Fit refinement worker, so an
    // over-sampled B1500A family would otherwise keep that action in flight for
    // multiple seconds. Even spacing over the concatenated per-curve points keeps
    // every gate proportionally represented; a smooth Id-Vd family is fully
    // captured well under the cap.
    const OUTPUT_FIT_SAMPLE_CAP: usize = 400;
    if samples.len() > OUTPUT_FIT_SAMPLE_CAP {
        let n = samples.len();
        samples = (0..OUTPUT_FIT_SAMPLE_CAP)
            .map(|k| samples[k * (n - 1) / (OUTPUT_FIT_SAMPLE_CAP - 1)])
            .collect();
    }
    let transfer_vgs = samples
        .iter()
        .map(|sample| sample.0)
        .max_by(f64::total_cmp)?;

    let seed = extract_output(curves, polarity.map_vg(params.vto), polarity);
    let asat0 = seed.map_or(params.asat, |p| p.alpha_sat).clamp(0.05, 5.0);
    let lambda0 = seed.map_or(params.lambda, |p| p.lambda).clamp(0.0, 0.5);
    let opts = LevMarOptions {
        max_iters: 250,
        ..LevMarOptions::default()
    };
    const VKINK_OFF: f64 = 1.0e6;
    let build_params = |asat: f64, lambda: f64, vkink: f64| {
        // ASAT and LAMBDA also set the saturated transfer amplitude. Preserve the
        // transfer-calibrated mobility×output-gain product while fitting Id-Vd
        // shape, otherwise an output attach silently invalidates the transfer fit.
        let old_gain = params.asat * (1.0 + params.lambda * transfer_vds);
        let new_gain = asat * (1.0 + lambda * transfer_vds);
        let mobility_scale = old_gain / new_gain;
        let mut candidate = Level62Params {
            asat,
            lambda,
            vkink,
            mu0: params.mu0 * mobility_scale,
            mu1: params.mu1 * mobility_scale,
            ..params
        };
        // A fitted kink also changes the transfer-bias current. Anchor the full
        // forward (not only its saturation product) at the strongest measured
        // gate so enabling/disabling VKINK cannot invalidate that calibration.
        let reference_anchor = level62_current(
            &Level62Params {
                vkink: params.vkink,
                ..candidate
            },
            geom,
            temp_k,
            transfer_vgs,
            transfer_vds,
        );
        let candidate_anchor =
            level62_current(&candidate, geom, temp_k, transfer_vgs, transfer_vds);
        if reference_anchor.is_finite()
            && reference_anchor > 0.0
            && candidate_anchor.is_finite()
            && candidate_anchor > 0.0
        {
            let scale = reference_anchor / candidate_anchor;
            candidate.mu0 *= scale;
            candidate.mu1 *= scale;
        }
        candidate
    };
    let base_build = |x: &[f64]| build_params(x[0], x[1], VKINK_OFF);
    let base_residual = |x: &[f64]| -> Vec<f64> {
        let p = base_build(x);
        samples
            .iter()
            .map(|&(vgs, vds, measured)| {
                level62_current(&p, geom, temp_k, vgs, vds)
                    .max(1.0e-30)
                    .log10()
                    - measured.log10()
            })
            .collect()
    };
    let base = levenberg_marquardt(
        base_residual,
        &[asat0, lambda0],
        Some((&[0.05, 0.0], &[5.0, 0.5])),
        &opts,
    );
    let baseline = base
        .cost
        .is_finite()
        .then(|| (base.cost, base_build(&base.params)));
    let lo = [0.05, 0.0, -1.0];
    let hi = [5.0, 0.5, 6.0];
    let build = |x: &[f64]| build_params(x[0], x[1], 10f64.powf(x[2]));
    let residual = |x: &[f64]| -> Vec<f64> {
        let p = build(x);
        samples
            .iter()
            .map(|&(vgs, vds, measured)| {
                level62_current(&p, geom, temp_k, vgs, vds)
                    .max(1.0e-30)
                    .log10()
                    - measured.log10()
            })
            .collect()
    };

    let kink_evidence = measured_kink_evidence(curves, &params, polarity);
    let mut best: Option<(f64, Vec<f64>)> = None;
    if kink_evidence {
        for vkink0 in [params.vkink, 2.0, 10.0, 100.0] {
            let x0 = [asat0, lambda0, vkink0.max(0.1).log10().clamp(lo[2], hi[2])];
            let out = levenberg_marquardt(residual, &x0, Some((&lo, &hi)), &opts);
            if out.cost.is_finite() && best.as_ref().is_none_or(|(cost, _)| out.cost < *cost) {
                best = Some((out.cost, out.params));
            }
        }
    }
    let (stage_one, kink_selected) = match (baseline, best) {
        (Some((base_cost, _)), Some((cost, x))) if cost < 0.99 * base_cost => (build(&x), true),
        (Some((_, base_params)), _) => (base_params, false),
        (None, Some((_, x))) => (build(&x), true),
        (None, None) => return None,
    };

    let tails = output_tail_targets(curves, polarity);
    if tails.len() < 2 {
        return Some(stage_one);
    }
    let current_scale = samples
        .iter()
        .map(|sample| sample.2)
        .fold(0.0_f64, f64::max);
    let tail_error = |p: &Level62Params| {
        (tails
            .iter()
            .map(|tail| {
                let model = level62_output(p, geom, temp_k, tail.vgs, &tail.vds);
                let slope = output_tail_slope(&tail.vds, &model)
                    .map(|value| value.0)
                    .unwrap_or(1.0e-30);
                (slope.max(1.0e-30) / tail.slope).log10().powi(2)
            })
            .sum::<f64>()
            / tails.len() as f64)
            .sqrt()
    };
    let output_error = |p: &Level62Params| {
        (samples
            .iter()
            .map(|&(vgs, vds, measured)| {
                (level62_current(p, geom, temp_k, vgs, vds) - measured).powi(2)
            })
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
            / current_scale
    };
    let family_peak = curves
        .iter()
        .flat_map(|curve| curve.id.iter())
        .map(|id| id.abs())
        .fold(0.0_f64, f64::max);
    let max_strong_curve_error = |p: &Level62Params| {
        curves
            .iter()
            .filter_map(|curve| {
                let vgs = polarity.map_vg(curve.vg);
                let points: Vec<_> = curve
                    .vds
                    .iter()
                    .zip(&curve.id)
                    .filter_map(|(&vd, &id)| {
                        let vds = s * vd;
                        let measured = id.abs();
                        (vds > 0.2 && measured.is_finite()).then_some((vds, measured))
                    })
                    .collect();
                let peak = points.iter().map(|point| point.1).fold(0.0_f64, f64::max);
                if points.is_empty() || peak < 0.2 * family_peak {
                    return None;
                }
                Some(
                    (points
                        .iter()
                        .map(|&(vds, measured)| {
                            (level62_current(p, geom, temp_k, vgs, vds) - measured).powi(2)
                        })
                        .sum::<f64>()
                        / points.len() as f64)
                        .sqrt()
                        / peak,
                )
            })
            .fold(0.0_f64, f64::max)
    };
    let transfer_r2 = |p: &Level62Params| {
        transfer_preservation_log_r2(transfer, |vg| {
            level62_current(p, geom, temp_k, polarity.map_vg(vg), transfer_vds)
        })
    };
    let append_tail_residual = |p: &Level62Params, result: &mut Vec<f64>| {
        result.extend(tails.iter().map(|tail| {
            let model = level62_output(p, geom, temp_k, tail.vgs, &tail.vds);
            let slope = output_tail_slope(&tail.vds, &model)
                .map(|value| value.0)
                .unwrap_or(1.0e-30);
            2.0 * (slope.max(1.0e-30) / tail.slope).log10()
        }));
    };
    let candidate = if kink_selected {
        let combined = |x: &[f64]| {
            let p = build(x);
            let mut result = residual(x);
            append_tail_residual(&p, &mut result);
            result
        };
        let x0 = [
            stage_one.asat,
            stage_one.lambda,
            stage_one.vkink.max(0.1).log10().clamp(lo[2], hi[2]),
        ];
        let out = levenberg_marquardt(combined, &x0, Some((&lo, &hi)), &opts);
        build(&out.params)
    } else {
        let combined = |x: &[f64]| {
            let p = base_build(x);
            let mut result = base_residual(x);
            append_tail_residual(&p, &mut result);
            result
        };
        let x0 = [stage_one.asat, stage_one.lambda];
        let out = levenberg_marquardt(combined, &x0, Some((&[0.05, 0.0], &[5.0, 0.5])), &opts);
        base_build(&out.params)
    };
    const MAX_STRONG_CURVE_NRMSE: f64 = 0.07;
    (tail_error(&candidate) <= 0.9 * tail_error(&stage_one)
        && output_error(&candidate) <= output_error(&stage_one) + 0.01
        && max_strong_curve_error(&candidate) <= MAX_STRONG_CURVE_NRMSE
        && max_strong_curve_error(&candidate) <= max_strong_curve_error(&stage_one) + 0.01
        && transfer_r2(&candidate) >= transfer_r2(&stage_one) - 0.005)
        .then_some(candidate)
        .or(Some(stage_one))
}
