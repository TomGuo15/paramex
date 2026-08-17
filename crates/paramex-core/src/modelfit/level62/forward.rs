use super::params::Level62Params;
use crate::modelfit::types::GeometryParams;

/// Vacuum permittivity (F·m⁻¹).
pub(super) const E0: f64 = 8.854_187_8e-12;
/// Elementary charge (C).
pub(super) const Q: f64 = 1.602_176_634e-19;
/// Boltzmann constant (J·K⁻¹).
pub(super) const KB: f64 = 1.380_649e-23;

/// The RPI effective-overdrive smooth clamp (same form as Level 61):
/// `(Vmin/2)·[1 + v/Vmin + sqrt(DELTA² + (v/Vmin − 1)²)]`. Stays positive for all `v`
/// (so the mobility and `Vdsat` are always defined) and → `v` for `v ≫ Vmin`.
fn vgte_clamp(v: f64, vmin: f64, delta: f64) -> f64 {
    0.5 * vmin * (1.0 + v / vmin + (delta * delta + (v / vmin - 1.0).powi(2)).sqrt())
}

/// Cap on the subthreshold gate exponent `VGT/Vsth` so `exp()` never overflows f64 (which
/// happens past ~709 and turns a current interpolation with large products into `NaN`). Chosen well
/// above the `Isub ≈ Ia` crossover (a few `Vsth` past threshold) so it only engages deep in
/// strong inversion where `Ichan → Ia` is already insensitive to `Isub`'s magnitude — i.e.
/// the cap changes the drain current by nothing it can resolve. The `.va` mirrors this value.
const SUBVT_EXP_CAP: f64 = 80.0;

/// Isothermal Level 62 drain current at one bias, in the n-channel-on frame
/// (`vgs` rises into conduction, `vds ≥ 0`). `temp_k` is the measurement temperature
/// (TEMP = TNOM). Returns `Ids` (A).
pub(super) fn level62_current(
    p: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    vgs: f64,
    vds: f64,
) -> f64 {
    let rs = p.rs.max(0.0);
    let rd = p.rd.max(0.0);
    let r_total = rs + rd;
    if r_total <= 0.0 || vds <= 0.0 {
        return level62_intrinsic_current(p, geom, temp_k, vgs, vds);
    }

    let ideal = level62_intrinsic_current(p, geom, temp_k, vgs, vds);
    let mut lo = 0.0;
    let mut hi = ideal.min(vds / r_total).max(0.0);
    if hi <= 0.0 || !hi.is_finite() {
        return ideal;
    }

    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let internal = level62_intrinsic_current(
            p,
            geom,
            temp_k,
            vgs - mid * rs,
            (vds - mid * r_total).max(0.0),
        );
        if mid > internal {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

fn level62_intrinsic_current(
    p: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    vgs: f64,
    vds: f64,
) -> f64 {
    let vth = KB * temp_k / Q;
    let vsth = (p.eta * vth).max(1.0e-9);
    let cox = p.epsi * E0 / p.tox;
    let w = geom.w_um * 1.0e-6;
    let l = (geom.l_um * 1.0e-6).max(1.0e-12);
    let wl = w / l;
    // ΔT = TEMP − TNOM; zero in-app (ParamEx evaluates at the measurement
    // temperature), live in the .va via $temperature.
    let dt = temp_k - p.tnom_k;

    // DIBL-shifted effective threshold (manual p. 273):
    // VTeff = VTX − (AT·Vds² + BT)/(Leff·(1 + exp((Vgs − VST − VTX)/VSI))).
    // The gate exponent is capped like SUBVT_EXP_CAP (exp(80) already makes the
    // shift indistinguishable from zero, and the .va exp() must not overflow).
    let vtx = p.vto - p.dvto * dt;
    let dibl_gate = 1.0
        + ((vgs - p.vst - vtx) / p.vsi.max(1.0e-9))
            .min(SUBVT_EXP_CAP)
            .exp();
    let vteff = vtx - (p.at * vds * vds + p.bt) / (l * dibl_gate);

    // Effective overdrive, smoothed.
    let vgt = vgs - vteff;
    let vmin = 2.0 * vsth;
    let vgte = vgte_clamp(vgt, vmin, p.delta);

    // Rise-then-saturate poly-Si field-effect mobility (→ MU0 at large overdrive),
    // with the µ1 temperature shift (no-op at ΔT = 0).
    let mu1t = p.mu1 + p.dmu1 * dt;
    let mufet = 1.0 / (1.0 / p.mu0 + 1.0 / (mu1t * (2.0 * vgte / vsth).powf(p.mmu)));

    // Saturation parameter with its length + temperature terms (αsat = ASAT at the
    // defaults; floored so the triode division stays finite).
    let asatt = (p.asat - p.lasat / l - p.dasat * dt).max(1.0e-6);

    // Above-threshold square-law current — the HSPICE per-regime form (VGTE + bare Vds,
    // switched at the knee Vds = σsat·VGTE; value and slope are continuous there).
    let vdsat = (asatt * vgte).max(1.0e-12);
    let ia_base = if vds <= vdsat {
        vgte * vds - vds * vds / (2.0 * asatt)
    } else {
        asatt * vgte * vgte / 2.0
    };
    // Keep the empirical CLM multiplier on both sides of the hard region switch.
    // Applying it only in saturation creates a current step at VDSAT and a false
    // one-point gds peak; the shared factor preserves both value and slope there.
    let ia_core = ia_base * (1.0 + p.lambda * vds);
    let ia = (mufet * cox * wl * ia_core).max(1.0e-30);

    // Exponential subthreshold (diffusion) current — Vsth in all three places. The gate
    // exponent argument is capped (the published `exp(VGT/Vsth)` overflows f64 to +inf at
    // high overdrive — `VGT/Vsth` passes ~709. The cap is behavior-preserving: above
    // threshold Isub ≫ Ia, so the stabilized blend → Ia regardless of how large the
    // (now-finite) Isub is, and the cap only
    // engages deep in strong inversion, far past the Isub≈Ia crossover. See the HSPICE
    // X-2005.09 Level 62 branch equation and ParamEx's stabilized order-1/2 blend.
    let isub = (p.mus
        * cox
        * wl
        * vsth
        * vsth
        * (vgt / vsth).min(SUBVT_EXP_CAP).exp()
        * (1.0 - (-vds / vsth).exp()))
    .max(1.0e-30);

    // Broad generalized harmonic interpolation preserves both current asymptotes while
    // spreading the branch-slope handoff over the moderate-inversion region.
    let lo = ia.min(isub);
    let ratio = (lo / ia.max(isub)).sqrt();
    let ichan = lo / (1.0 + ratio).powi(2);

    // Impact-ionization kink: only turns on for Vds well past pinch-off.
    let vdsk = vds / (1.0 + (vds / vdsat).powi(3)).powf(1.0 / 3.0) - vth;
    let excess = vds - vdsk;
    let ikink = if excess > 1.0e-9 {
        (1.0 / p.vkink) * (p.lkink / l).powf(p.mk) * excess * (-p.vkink / excess).exp()
    } else {
        0.0
    };

    // Off-state leakage floor (reverse-diode branch; the field-emission XTFE blend is
    // deferred — it needs the off-region Vds-field dependence a transfer does not give).
    let ileak = (p.i00 * w * (-p.eb / vth).exp() * (1.0 - (-vds / vth).exp())).max(0.0);

    (ichan + ileak) * (1.0 + ikink)
}

/// Transfer sweep: `Ids` vs `vgs` at fixed `vds`.
pub(in crate::modelfit) fn level62_transfer(
    p: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    vgs: &[f64],
    vds: f64,
) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| level62_current(p, geom, temp_k, vg, vds))
        .collect()
}

/// Output sweep: `Ids` vs `vds` at fixed `vgs` (exercises the kink).
pub(in crate::modelfit) fn level62_output(
    p: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    vgs: f64,
    vds: &[f64],
) -> Vec<f64> {
    vds.iter()
        .map(|&vd| level62_current(p, geom, temp_k, vgs, vd))
        .collect()
}
