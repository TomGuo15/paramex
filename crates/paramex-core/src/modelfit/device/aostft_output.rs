//! AOSTFT-specific nonlinear output refinement.

use crate::modelfit::extract::{output_tail_slope, output_tail_targets, AboveThresholdFit};
use crate::modelfit::forward::output_card_current;
use crate::modelfit::optimize::{levenberg_marquardt, LevMarOptions};
use crate::modelfit::transfer_preservation_log_r2;
use crate::modelfit::types::{BiasParams, OutputCurve, OutputParams, Polarity, SubthresholdParams};

/// Guarded analog refinement of the closed-form AOSTFT output parameters. The
/// H-fit remains the transfer calibration; every candidate derives its card K
/// from that same calibration before its Id-Vd and gds residuals are evaluated.
pub(super) fn refine_output(
    seed: OutputParams,
    curves: &[OutputCurve],
    transfer: (&[f64], &[f64]),
    h_fit: AboveThresholdFit,
    sub: SubthresholdParams,
    bias: BiasParams,
    polarity: Polarity,
) -> Option<OutputParams> {
    let tails = output_tail_targets(curves, polarity);
    if tails.len() < 2 || !(bias.v_ds.is_finite() && bias.v_ds > 0.0) {
        return None;
    }
    let vt = polarity.map_vg(h_fit.vt);
    let gamma = h_fit.gamma - 1.0;
    if !gamma.is_finite() || gamma <= -1.0 {
        return None;
    }
    let s = polarity.sign();
    let mut samples = Vec::new();
    for curve in curves {
        let vgs = polarity.map_vg(curve.vg);
        for (&vd, &id) in curve.vds.iter().zip(&curve.id) {
            let (vds, id) = (s * vd, id.abs());
            if vds > 0.0 && id > 0.0 && vds.is_finite() && id.is_finite() {
                samples.push((vgs, vds, id));
            }
        }
    }
    if samples.len() < 20 {
        return None;
    }
    const SAMPLE_CAP: usize = 400;
    if samples.len() > SAMPLE_CAP {
        let n = samples.len();
        samples = (0..SAMPLE_CAP)
            .map(|i| samples[i * (n - 1) / (SAMPLE_CAP - 1)])
            .collect();
    }
    let current_scale = samples
        .iter()
        .map(|sample| sample.2)
        .fold(0.0_f64, f64::max);
    let build = |x: &[f64]| OutputParams {
        alpha_sat: x[0],
        lambda: x[1],
        m: x[2],
    };
    let predict = |p: OutputParams, vgs: f64, vds: &[f64]| {
        let gain = p.alpha_sat * (1.0 + p.lambda * bias.v_ds);
        output_card_current(h_fit.k / gain, gamma, bias.r, &p, &sub, vgs - vt, vds)
    };
    let tail_error = |p: OutputParams| {
        (tails
            .iter()
            .map(|tail| {
                let model = predict(p, tail.vgs, &tail.vds);
                let slope = output_tail_slope(&tail.vds, &model)
                    .map(|value| value.0)
                    .unwrap_or(1.0e-30);
                (slope.max(1.0e-30) / tail.slope).log10().powi(2)
            })
            .sum::<f64>()
            / tails.len() as f64)
            .sqrt()
    };
    let output_error = |p: OutputParams| {
        (samples
            .iter()
            .map(|&(vgs, vds, measured)| (predict(p, vgs, &[vds])[0] - measured).powi(2))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
            / current_scale
    };
    let transfer_r2 = |p: OutputParams| {
        transfer_preservation_log_r2(transfer, |vg| {
            predict(p, polarity.map_vg(vg), &[bias.v_ds])[0]
        })
    };
    let residual = |x: &[f64]| {
        let p = build(x);
        let mut result = Vec::with_capacity(2 * samples.len() + tails.len());
        for &(vgs, vds, measured) in &samples {
            let model = predict(p, vgs, &[vds])[0];
            result.push(model.max(1.0e-30).log10() - measured.log10());
            result.push((model - measured) / current_scale);
        }
        result.extend(tails.iter().map(|tail| {
            let model = predict(p, tail.vgs, &tail.vds);
            let slope = output_tail_slope(&tail.vds, &model)
                .map(|value| value.0)
                .unwrap_or(1.0e-30);
            2.0 * (slope.max(1.0e-30) / tail.slope).log10()
        }));
        result
    };
    let baseline_tail = tail_error(seed);
    let baseline_output = output_error(seed);
    let baseline_transfer = transfer_r2(seed);
    let out = levenberg_marquardt(
        residual,
        &[seed.alpha_sat, seed.lambda, seed.m],
        Some((&[0.05, 0.0, 0.2], &[5.0, 0.5, 20.0])),
        &LevMarOptions {
            max_iters: 250,
            ..LevMarOptions::default()
        },
    );
    let candidate = build(&out.params);
    let candidate_tail = tail_error(candidate);
    let candidate_output = output_error(candidate);
    let candidate_transfer = transfer_r2(candidate);
    (candidate_tail <= 0.9 * baseline_tail
        && candidate_output <= baseline_output + 0.01
        && candidate_transfer >= baseline_transfer - 0.005)
        .then_some(candidate)
}
