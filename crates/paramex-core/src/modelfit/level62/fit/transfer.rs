use super::super::forward::{level62_current, level62_transfer, E0, KB, Q};
use super::super::params::Level62Params;
use crate::modelfit::extract::{prepare_transfer, FIT_POINT_CAP};
use crate::modelfit::optimize::{levenberg_marquardt, LevMarOptions};
use crate::modelfit::types::{GeometryParams, Polarity};

/// Broad physical ceiling for fitted subthreshold mobility (50 cm²/Vs). Values
/// above this let the exponential branch overshoot the field-effect branch and
/// create a false gm peak at their crossover.
const MAX_FITTED_MUS: f64 = 5.0e-3;

/// Result of a Level 62 extraction: the fitted parameters (n-channel extraction
/// frame), the detected channel [`Polarity`], the overlay R² on `log10(Id)`, and
/// whether the LM refinement converged.
#[derive(Debug, Clone, PartialEq)]
pub struct Level62Fit {
    /// Fitted Level 62 parameters (n-channel frame; the export slice folds polarity).
    pub params: Level62Params,
    /// Detected channel polarity of the measured device.
    pub polarity: Polarity,
    /// R² of the model overlay vs the measured transfer, on `log10(Id)`.
    pub r2: f64,
    /// Whether the Levenberg–Marquardt refinement met a convergence test.
    pub converged: bool,
}

/// Overlay R² on `log10(Id)` for a candidate parameter set over the measured transfer.
/// `ys` is the precomputed `log10` of the measured current and `sst` its total sum of
/// squares about the mean.
fn log_overlay_r2(
    p: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    v_ds: f64,
    vgn: &[f64],
    ys: &[f64],
    sst: f64,
) -> f64 {
    let sr: f64 = vgn
        .iter()
        .enumerate()
        .map(|(k, &v)| {
            (level62_current(p, geom, temp_k, v, v_ds)
                .max(1.0e-30)
                .log10()
                - ys[k])
                .powi(2)
        })
        .sum();
    1.0 - sr / sst
}

/// Extract Level 62 (LTPS) parameters from a measured transfer sweep:
/// data-driven seeds for the transfer-identifiable set `{VTO, ETA, MU0, MU1, MMU, MUS}`
/// (ETA from the subthreshold slope, MU0 from the high-overdrive effective mobility,
/// MUS from the subthreshold magnitude), then a **multi-start** Levenberg–Marquardt
/// refine on `log10(Id)` — the full-curve overlay objective, multi-start over the
/// correlated `(VTO, MU1)` basin (a robustness lesson from earlier model
/// extractions). `template` supplies the fixed material/kink/leakage
/// constants; `Cox` is fixed from `EPSI`/`TOX`. `v_ds` is the transfer drain bias.
/// `None` if no usable on-region exists or every start fails. The kink/DIBL/leakage/
/// temperature parameters are NOT identifiable from a transfer and stay at `template`.
pub(in crate::modelfit) fn extract_level62(
    vg: &[f64],
    id_abs: &[f64],
    geom: GeometryParams,
    v_ds: f64,
    temp_k: f64,
    template: Level62Params,
) -> Option<Level62Fit> {
    let prepared = prepare_transfer(vg, id_abs)?.decimated(FIT_POINT_CAP);
    let (vgn, idn, polarity) = (prepared.vg(), prepared.id(), prepared.polarity());
    let n = vgn.len().min(idn.len());
    if n < 10 {
        return None;
    }
    let vth = KB * temp_k / Q;
    let cox = template.epsi * E0 / template.tox;
    let wl = geom.w_um / geom.l_um.max(f64::MIN_POSITIVE);

    let mut pos: Vec<f64> = idn.iter().copied().filter(|x| *x > 0.0).collect();
    pos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if pos.len() < 5 {
        return None;
    }
    let idmax = pos[pos.len() - 1];

    // ETA seed from the steepest subthreshold log-slope (SS = 1/slope V/dec → ETA), and
    // vg_knee marking the turn-on neighbourhood (the VTO seed centre).
    let (mut max_slope, mut vg_knee) = (0.0_f64, vgn[n / 2]);
    for i in 1..n {
        if idn[i] > 0.0 && idn[i - 1] > 0.0 && idn[i] < 0.5 * idmax {
            let dv = vgn[i] - vgn[i - 1];
            if dv.abs() > 1.0e-9 {
                let s = (idn[i].log10() - idn[i - 1].log10()) / dv;
                if s > max_slope {
                    max_slope = s;
                    vg_knee = 0.5 * (vgn[i] + vgn[i - 1]);
                }
            }
        }
    }
    let ss = if max_slope > 0.0 {
        1.0 / max_slope
    } else {
        1.0
    };
    let eta_seed = (ss / (std::f64::consts::LN_10 * vth)).clamp(1.0, 30.0);

    // MU0 seed: effective field-effect mobility at the top of the sweep.
    let mu0_seed =
        (idmax / (cox * wl * (vgn[n - 1] - vg_knee).max(0.5) * v_ds)).clamp(1.0e-4, 5.0e-2);

    // MUS seed from a mid-subthreshold point.
    let vsth_s = eta_seed * vth;
    let mut mus_seed = template.mus;
    if let Some(i) = (0..n).find(|&i| idn[i] >= 0.01 * idmax) {
        let vgt = vgn[i] - vg_knee;
        let denom =
            cox * wl * vsth_s * vsth_s * (vgt / vsth_s).exp() * (1.0 - (-v_ds / vsth_s).exp());
        if denom > 0.0 {
            mus_seed = (idn[i] / denom).clamp(1.0e-6, MAX_FITTED_MUS);
        }
    }

    // Seed the reverse-diode prefactor I00 so the model's off-FLOOR matches the measured
    // off-current (robust low-tenth median), instead of an arbitrary default — a mismatched
    // floor otherwise drags the log-overlay R² across the whole subthreshold region. (The
    // gate-leakage upturn at the far off-end is NOT channel current and is not modelled.)
    let ioff = pos[(pos.len() / 10).max(3) / 2];
    let diode_unit =
        (geom.w_um * 1.0e-6 * (-template.eb / vth).exp() * (1.0 - (-v_ds / vth).exp()))
            .max(1.0e-300);
    let i00_seed = (ioff / diode_unit).clamp(1.0e-3, 1.0e8);
    let template = Level62Params {
        i00: i00_seed,
        ..template
    };

    // Precompute the log10 target for the overlay R².
    let ys: Vec<f64> = idn.iter().map(|i| i.max(1.0e-30).log10()).collect();
    let ybar = ys.iter().sum::<f64>() / ys.len() as f64;
    let sst: f64 = ys.iter().map(|y| (y - ybar).powi(2)).sum();

    // LM free vector x = [VTO, ETA, log10 MU0, log10 MU1, MMU, log10 MUS].
    let build = |x: &[f64]| Level62Params {
        vto: x[0],
        eta: x[1],
        mu0: 10f64.powf(x[2]),
        mu1: 10f64.powf(x[3]),
        mmu: x[4],
        mus: 10f64.powf(x[5]),
        ..template
    };
    let residual = |x: &[f64]| -> Vec<f64> {
        let p = build(x);
        vgn.iter()
            .zip(idn)
            .map(|(&v, &i)| {
                level62_current(&p, geom, temp_k, v, v_ds)
                    .max(1.0e-30)
                    .log10()
                    - i.max(1.0e-30).log10()
            })
            .collect()
    };
    // MMU is a positive mobility-shape exponent; Level 62 does not require it
    // to be >= 1. Keeping that old artificial floor forces shallow-mobility
    // devices into a visibly wrong gate-family shape.
    let lo = [
        vg_knee - 4.0,
        1.0,
        (1.0e-4_f64).log10(),
        (1.0e-9_f64).log10(),
        0.05,
        (1.0e-6_f64).log10(),
    ];
    let hi = [
        vg_knee + 4.0,
        30.0,
        (1.0e-1_f64).log10(),
        (1.0e-3_f64).log10(),
        4.0,
        MAX_FITTED_MUS.log10(),
    ];
    let clamp = |v: f64, k: usize| v.clamp(lo[k], hi[k]);
    let opts = LevMarOptions {
        max_iters: 400,
        ..LevMarOptions::default()
    };

    // Multi-start over (VTO, MU1) — the correlated mobility basin is local-minima-prone.
    let mut best: Option<(f64, Vec<f64>, bool)> = None;
    for &dvto in &[-1.0, 0.0, 1.0] {
        for &m1log in &[template.mu1.log10(), -6.0] {
            let x0 = [
                clamp(vg_knee + dvto, 0),
                clamp(eta_seed, 1),
                clamp(mu0_seed.log10(), 2),
                clamp(m1log, 3),
                clamp(template.mmu, 4),
                clamp(mus_seed.log10(), 5),
            ];
            let out = levenberg_marquardt(residual, &x0, Some((&lo, &hi)), &opts);
            let r = log_overlay_r2(&build(&out.params), geom, temp_k, v_ds, vgn, &ys, sst);
            if r.is_finite() && best.as_ref().is_none_or(|(br, ..)| r > *br) {
                best = Some((r, out.params, out.converged));
            }
        }
    }
    let (mut r2, mut xbest, mut converged) = best?;
    if !r2.is_finite() {
        return None;
    }

    // A log-current overlay can be excellent while its derivative is wrong in
    // the analog on-region. Give the measured gm shape one guarded refinement
    // from the best DC solution, using the same four-step secant on measured and
    // modelled current so point noise is not fitted as a derivative. The DC fit
    // remains authoritative: accept only an actual gm improvement with at most
    // a small loss of full-range log-overlay R².
    const GM_RESIDUAL_WEIGHT: f64 = 5.0;
    const MAX_ANALOG_R2_LOSS: f64 = 0.005;
    let id_floor = idn.iter().copied().fold(f64::INFINITY, f64::min);
    let id_peak = idn.iter().copied().fold(0.0_f64, f64::max);
    let on_current = id_floor + 0.01 * (id_peak - id_floor);
    let mut gm_targets = Vec::new();
    for i in 2..n.saturating_sub(2) {
        let (lo_i, hi_i) = (i - 2, i + 2);
        let dv = vgn[hi_i] - vgn[lo_i];
        if dv > 0.0 && idn[i] >= on_current {
            let gm = (idn[hi_i] - idn[lo_i]) / dv;
            if gm.is_finite() && gm > 0.0 {
                gm_targets.push((lo_i, hi_i, dv, gm));
            }
        }
    }
    let mut measured_gm: Vec<_> = gm_targets.iter().map(|target| target.3).collect();
    measured_gm.sort_by(f64::total_cmp);
    let gm_scale = measured_gm
        .get(((measured_gm.len().saturating_sub(1) as f64) * 0.95).round() as usize)
        .copied()
        .unwrap_or(0.0);
    gm_targets.retain(|target| target.3 >= 0.05 * gm_scale);

    if gm_scale > 0.0 && gm_targets.len() >= 5 {
        let gm_error = |x: &[f64]| {
            let p = build(x);
            let model = level62_transfer(&p, geom, temp_k, vgn, v_ds);
            (gm_targets
                .iter()
                .map(|&(lo_i, hi_i, dv, measured)| {
                    let predicted = (model[hi_i] - model[lo_i]) / dv;
                    let scale = (gm_scale * measured.max(0.1 * gm_scale)).sqrt();
                    ((predicted - measured) / scale).powi(2)
                })
                .sum::<f64>()
                / gm_targets.len() as f64)
                .sqrt()
        };
        let baseline_gm_error = gm_error(&xbest);
        let analog_residual = |x: &[f64]| -> Vec<f64> {
            let p = build(x);
            let model = level62_transfer(&p, geom, temp_k, vgn, v_ds);
            let mut result: Vec<_> = model
                .iter()
                .zip(idn)
                .map(|(&predicted, &measured)| {
                    predicted.max(1.0e-30).log10() - measured.max(1.0e-30).log10()
                })
                .collect();
            result.extend(gm_targets.iter().map(|&(lo_i, hi_i, dv, measured)| {
                let predicted = (model[hi_i] - model[lo_i]) / dv;
                let scale = (gm_scale * measured.max(0.1 * gm_scale)).sqrt();
                GM_RESIDUAL_WEIGHT * (predicted - measured) / scale
            }));
            result
        };
        let analog = levenberg_marquardt(analog_residual, &xbest, Some((&lo, &hi)), &opts);
        let analog_r2 = log_overlay_r2(&build(&analog.params), geom, temp_k, v_ds, vgn, &ys, sst);
        let analog_gm_error = gm_error(&analog.params);
        if analog_r2.is_finite()
            && analog_r2 >= r2 - MAX_ANALOG_R2_LOSS
            && analog_gm_error.is_finite()
            && analog_gm_error < baseline_gm_error
        {
            r2 = analog_r2;
            xbest = analog.params;
            converged = analog.converged;
        }
    }

    Some(Level62Fit {
        params: build(&xbest),
        polarity,
        r2,
        converged,
    })
}
