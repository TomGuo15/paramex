//! AOSTFT forward model: parameters -> drain current. Built first so it is the
//! source of truth for synthetic data and the fit overlay.

#[cfg(test)]
use super::types::ModelParams;
use super::types::{OutputParams, SubthresholdParams};

/// Smallest saturation voltage allowed as a divisor, matching the exported `.va`
/// guard `max(vdsat, 1e-9)` so the overlay reproduces the card near `Vov = 0`.
const VDSAT_FLOOR: f64 = 1.0e-9;

/// `ln(10)`, converting the decade-based subthreshold swing into the natural
/// exponential of the subthreshold current.
const LN10: f64 = std::f64::consts::LN_10;

/// Overdrive smoothing width (V) for the effective gate overdrive `Vgte`, shared
/// by the overlay forward model and the exported `.va` so they agree. Small vs a
/// typical V_GT swing; makes the on-region power law and its derivatives C∞.
pub(super) const VGTE_SMOOTH_V: f64 = 5.0e-2;

/// Turn-on width (V) for the Meyer gate CHARGE only (AC/transient; DC current is
/// untouched). It is wider than `VGTE_SMOOTH_V` so Cgg co-locates with the
/// derivative-matched gm crossover instead of turning on ahead of it and gouging
/// a notch into fT = gm/(2π·Cgg). The off-state caps still vanish through the
/// smooth positive-part tail.
/// ponytail: fixed width; thread S for a per-device match if it ever matters.
pub(super) const CHARGE_SMOOTH_V: f64 = 0.2;

/// Smooth positive part `(x + sqrt(x² + δ²))/2` — a C∞ replacement for
/// `max(x, 0)`: `→ x` for `x ≫ δ`, `→ 0` for `x ≪ −δ`, with no kink at 0. Used as
/// the effective gate overdrive `Vgte` so `gm`/capacitances are continuous.
pub(super) fn smooth_overdrive(vgt: f64, delta: f64) -> f64 {
    0.5 * (vgt + (vgt * vgt + delta * delta).sqrt())
}

/// Above-threshold transfer-curve drain current at each gate voltage:
/// `Id = K * (Vg - VT)^(1 + gamma)` for `Vg > VT`, else `0`.
#[cfg(test)]
pub(super) fn transfer_curve(params: &ModelParams, vgs: &[f64]) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| {
            let overdrive = vg - params.vt;
            if overdrive > 0.0 {
                params.k * overdrive.powf(1.0 + params.gamma)
            } else {
                0.0
            }
        })
        .collect()
}

/// Unified transfer current with a real off state, following the published
/// AOSTFT/UMEM model (Iñiguez et al., IEEE JEDS 2021, Eq. 28/29): the
/// above-threshold branch `I_DSA` is `tanh`-blended with an exponential
/// subthreshold branch `I_DSB = Ids0·exp(2.3·(Vg−VT)/S)` of slope = the
/// subthreshold swing `S`. `Ids0` is anchored for continuity at the crossover
/// `VT+DV`. An explicit `Ioff` leakage floor is added on top — the paper notes
/// gate leakage "was not considered", so we model it as the constant the
/// extraction recovers. The curve never hard-zeroes below VT.
///
/// `DV` and `Q` are the paper's blend offset/slope. `DV` is chosen so the
/// exponential and power-law branches meet in both value and first derivative;
/// `Q = 2/S` keeps the blend numerically stable above VT.
pub(super) fn unified_transfer(
    vt: f64,
    gamma: f64,
    k: f64,
    sub: &SubthresholdParams,
    vgs: &[f64],
) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| unified_transfer_at(vt, gamma, k, sub, vg))
        .collect()
}

fn unified_transfer_at(vt: f64, gamma: f64, k: f64, sub: &SubthresholdParams, vg: f64) -> f64 {
    let vgt = vg - vt;
    let s = sub.ss_v_dec.max(f64::MIN_POSITIVE);
    let io = sub.ioff.max(0.0);
    let (dv, q) = blend_params(s, 1.0 + gamma);

    // I_DSA: above-threshold (saturation transfer) power law on the smooth
    // effective overdrive (matches the exported .va, C∞ through threshold).
    let iab = |over: f64| k * smooth_overdrive(over, VGTE_SMOOTH_V).powf(1.0 + gamma);
    // I_DSB: exponential subthreshold, anchored at the crossover so it meets
    // I_DSA there (Ids0 = I_DSA(VT+DV)·exp(-2.3·DV/S)).
    let ids0 = iab(dv) * (-LN10 * dv / s).exp();

    // Eq. 29 tanh blend, computed stably (sigmoid + softplus) to avoid overflow.
    let w = (vgt - dv) * q;
    let above = iab(vgt) / (1.0 + (-2.0 * w).exp());
    let softplus_2w = (2.0 * w).max(0.0) + (1.0 + (-(2.0 * w).abs()).exp()).ln();
    let below = ids0 * (LN10 * vgt / s - softplus_2w).exp();
    above + below + io
}

/// AOSTFT blend offset `DV` and slope `Q`, derived from subthreshold swing `S`
/// and the on-branch overdrive power. For `I_on ∝ smooth(Vgt)^power`,
/// `d(ln I_on)/dVgt = power/sqrt(Vgt²+δ²)`; matching it to the exponential
/// slope `ln(10)/S` at `DV` removes the value-only crossover's artificial gm
/// peak/dip. `Q = 2/S` keeps the subthreshold branch stable above threshold.
pub(super) fn blend_params(s: f64, power: f64) -> (f64, f64) {
    let radius = power.max(0.0) * s / LN10;
    let dv = (radius * radius - VGTE_SMOOTH_V * VGTE_SMOOTH_V)
        .max(0.0)
        .sqrt();
    (dv, 2.0 / s)
}

/// Full DC drain current of the EXPORTED `.va`/`.scs` card at a fixed gate
/// overdrive, swept over `vds` — the single source of truth for the FIT OVERLAY's
/// output overlay and the predicted output family, so the plot reproduces the
/// exported card to ULP across the whole `Vd`/`Vg` range (not just well above
/// threshold). Mirrors `verilog_a_card`'s analog block exactly:
/// `ids = above + below + IOFF`, where the strict-Eq.25 above-threshold term
/// `iab = gch·Vdse·(1+λ·Vd)` is `tanh`-blended (overflow-safe sigmoid/softplus
/// form) with the exponential subthreshold branch anchored for continuity at
/// `VTO+DV`. Inputs are in the on-direction frame (`vgt = Vov ≥ 0` when on,
/// `vds ≥ 0`); the caller folds polarity. `g0` is the conductance gain
/// (`K/V_DS`); `rs` the series resistance.
pub(super) fn output_card_current(
    g0: f64,
    gamma: f64,
    rs: f64,
    out: &OutputParams,
    sub: &SubthresholdParams,
    vgt: f64,
    vds: &[f64],
) -> Vec<f64> {
    let s = sub.ss_v_dec.max(f64::MIN_POSITIVE);
    let io = sub.ioff.max(0.0);
    // The strict finite-Vd card is anchored from a saturation transfer, whose
    // on-current power is 2+gamma (gch contributes 1+gamma, Vdsat one more).
    let (dv, qb) = blend_params(s, 2.0 + gamma);
    // Series-R-corrected conductance from a smooth overdrive (matches the .va).
    let gch = |over: f64| {
        let g = g0 * smooth_overdrive(over, VGTE_SMOOTH_V).powf(1.0 + gamma);
        g / (1.0 + rs * g)
    };
    let vdsat = out.alpha_sat * smooth_overdrive(vgt, VGTE_SMOOTH_V);
    let vdsatc = out.alpha_sat * smooth_overdrive(dv, VGTE_SMOOTH_V);
    let g_on = gch(vgt);
    let g_anchor = gch(dv);
    // Blend weight + softplus depend only on the gate overdrive (constant per curve).
    let w = (vgt - dv) * qb;
    let above_w = 1.0 / (1.0 + (-2.0 * w).exp());
    let sp2w = (2.0 * w).max(0.0) + (1.0 + (-(2.0 * w).abs()).exp()).ln();
    let vdse = |vd: f64, vsat: f64| {
        vd / (1.0 + (vd / vsat.max(VDSAT_FLOOR)).abs().powf(out.m)).powf(1.0 / out.m)
    };
    vds.iter()
        .map(|&vd| {
            let iab = g_on * vdse(vd, vdsat) * (1.0 + out.lambda * vd);
            let iabc = g_anchor * vdse(vd, vdsatc) * (1.0 + out.lambda * vd);
            let ids0 = iabc * (-LN10 * dv / s).exp();
            let above = iab * above_w;
            let below = ids0 * (LN10 * vgt / s - sp2w).exp();
            above + below + io
        })
        .collect()
}

/// Output-curve drain current at each drain voltage, at a fixed `vg`:
/// `Id = g_ch * Vd_eff * (1 + lambda*Vd)` with the smoothed effective drain
/// `Vd_eff = Vd / (1 + (Vd/Vdsat)^m)^(1/m)` and `Vdsat = alpha_sat*(Vg - VT)`.
/// Zero below threshold or at `Vd <= 0`. `g_ch` is the channel conductance.
#[cfg(test)]
pub(super) fn output_curve(vt: f64, p: &OutputParams, g_ch: f64, vg: f64, vds: &[f64]) -> Vec<f64> {
    let vov = vg - vt;
    if vov <= 0.0 {
        return vec![0.0; vds.len()];
    }
    let vdsat = p.alpha_sat * vov;
    vds.iter()
        .map(|&vd| {
            if vd <= 0.0 {
                return 0.0;
            }
            let vd_eff = vd / (1.0 + (vd / vdsat).powf(p.m)).powf(1.0 / p.m);
            g_ch * vd_eff * (1.0 + p.lambda * vd)
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/forward.rs"]
mod integration_tests;
