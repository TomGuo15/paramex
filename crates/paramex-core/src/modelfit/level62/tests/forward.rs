use super::*;

/// The transfer current rises monotonically with gate (no spurious dips / kink in the
/// Id-Vg — the stabilized current blend is smooth, unlike a-Si Level 61's
/// charge crossover).
#[test]
fn level62_transfer_rises_monotonically_with_gate() {
    let p = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let vg: Vec<f64> = (0..=120).map(|i| -3.0 + 0.1 * i as f64).collect();
    let id = level62_transfer(&p, unit_geom(), T_NOM_K, &vg, 0.1);
    for w in id.windows(2) {
        assert!(
            w[1] >= w[0] * 0.999,
            "transfer should rise monotonically: {} -> {}",
            w[0],
            w[1]
        );
    }
    // Real dynamic range: off-floor well below the on-current.
    assert!(*id.last().unwrap() > 1.0e4 * id[0], "on/off dynamic range");
}

/// The output current saturates with drain voltage (the Vdse knee), then the KINK
/// lifts it back up at high Vds — the poly-Si signature absent in Level 61.
#[test]
fn level62_output_saturates_then_kinks() {
    let p = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let geom = unit_geom();
    let vgs = 6.0;
    // Knee region: current rises with Vds.
    let near = level62_output(&p, geom, T_NOM_K, vgs, &[0.5, 1.0, 2.0]);
    assert!(
        near[2] > near[1] && near[1] > near[0],
        "rises through the knee"
    );
    // Past saturation the bare square-law would plateau; the kink makes Id climb.
    let hi = level62_output(&p, geom, T_NOM_K, vgs, &[8.0, 20.0]);
    assert!(
        hi[1] > hi[0] * 1.05,
        "kink lifts the saturation current at high Vds: {} -> {}",
        hi[0],
        hi[1]
    );
}

#[test]
fn level62_current_is_continuous_at_vdsat_with_lambda() {
    let p = Level62Params {
        vto: 1.0,
        lambda: 0.02,
        vkink: 1.0e9,
        ..Level62Params::ltps()
    };
    let vgs = 5.0;
    let vth = KB * T_NOM_K / Q;
    let vsth = p.eta * vth;
    let vmin = 2.0 * vsth;
    let vgt = vgs - p.vto;
    let ratio = vgt / vmin;
    let vgte = 0.5 * vmin * (1.0 + ratio + (p.delta * p.delta + (ratio - 1.0).powi(2)).sqrt());
    let vdsat = p.asat * vgte;
    let step = 1.0e-7;
    let below = level62_current(&p, unit_geom(), T_NOM_K, vgs, vdsat - step);
    let above = level62_current(&p, unit_geom(), T_NOM_K, vgs, vdsat + step);
    let relative_jump = (above - below).abs() / below;
    assert!(
        relative_jump < 1.0e-5,
        "Level 62 current jumps {:.3}% at VDSAT={vdsat}",
        100.0 * relative_jump
    );
}

/// The KINK is a genuine model term: at high Vds, turning it on (finite VKINK) raises
/// the current above the kink-free (VKINK→∞) square-law saturation.
#[test]
fn level62_kink_raises_current_only_in_saturation() {
    let on = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let off = Level62Params { vkink: 1.0e9, ..on }; // VKINK→∞ disables the kink
    let geom = unit_geom();
    // In saturation (high Vds) the kink lifts the current.
    let id_on = level62_current(&on, geom, T_NOM_K, 6.0, 20.0);
    let id_off = level62_current(&off, geom, T_NOM_K, 6.0, 20.0);
    assert!(
        id_on > id_off * 1.05,
        "kink raises saturation current: {id_off:e} -> {id_on:e}"
    );
    // In the linear region (low Vds) the kink is negligible.
    let lin_on = level62_current(&on, geom, T_NOM_K, 6.0, 0.1);
    let lin_off = level62_current(&off, geom, T_NOM_K, 6.0, 0.1);
    assert!(
        (lin_on - lin_off).abs() < 1.0e-3 * lin_off,
        "kink off in the linear region"
    );
}

/// The poly-Si field-effect mobility RISES with gate bias (grain-boundary-barrier
/// lowering) before saturating — the opposite of a simple degradation. Checked via the
/// linear-region effective mobility µ_eff ∝ Id/(VGTE·Vds) increasing with Vgs.
#[test]
fn level62_mobility_rises_with_gate() {
    let p = Level62Params {
        vto: 0.0,
        ..Level62Params::ltps()
    };
    let geom = unit_geom();
    let vds = 0.05; // deep linear region
    let mu_eff = |vgs: f64| {
        let id = level62_current(&p, geom, T_NOM_K, vgs, vds);
        id / vgs // Id/Vgs ∝ µ_eff·VGTE/Vgs·Vds (VGTE≈Vgs above threshold) → tracks µ_eff
    };
    // From low to moderate overdrive the effective mobility climbs.
    assert!(
        mu_eff(4.0) > mu_eff(1.5) * 1.1,
        "µ_eff rises with gate bias"
    );
}

/// Independent hand-derivation of `level62_current` at one bias, from the manual
/// equations, pins the arithmetic (VGTE smoothing, rise-then-saturate mobility, the
/// square-law/subthreshold stabilized blend) end-to-end.
#[test]
fn level62_current_matches_independent_hand_derivation() {
    let p = Level62Params::default(); // VTO=0, MU0=1e-2, MU1=2.2e-7, MMU=3, ASAT=1, ETA=7, DELTA=4 …
    let geom = unit_geom();
    let (vgs, vds) = (5.0, 0.1);

    // Hand calculation (SI), independent of the production code path:
    let vth = KB * T_NOM_K / Q; // 0.025693
    let vsth = 7.0 * vth; // ETA·Vth = 0.179852
    let cox = 3.9 * E0 / 1.0e-7; // EPSI·ε0/TOX = 3.4531e-4
    let vmin = 2.0 * vsth;
    let r = (vgs - 0.0) / vmin;
    let vgte = 0.5 * vmin * (1.0 + r + (16.0 + (r - 1.0).powi(2)).sqrt()); // ≈ 5.1095
    let mufet = 1.0 / (1.0 / 1.0e-2 + 1.0 / (2.2e-7 * (2.0 * vgte / vsth).powi(3))); // ≈ 8.015e-3
                                                                                     // Triode (Vds=0.1 ≪ Vdsat≈5.11): Ia = µFET·Cox·(W/L)·(VGTE·Vds − Vds²/(2·σsat)), σsat=W/L=1.
    let ia = mufet * cox * (vgte * vds - vds * vds / 2.0);
    // Isub ≫ Ia here, so the stabilized blend ≈ Ia.
    let expected = ia; // ≈ 1.40e-6 A

    let got = level62_current(&p, geom, T_NOM_K, vgs, vds);
    let rel = (got - expected).abs() / expected;
    assert!(
        rel < 0.02,
        "hand-derivation: got {got:e} vs {expected:e} (rel {rel:e})"
    );
    assert!(
        (1.30e-6..1.50e-6).contains(&got),
        "sane magnitude (~1.4 µA): {got:e}"
    );
}

#[test]
fn level62_saturation_branch_matches_independent_hand_derivation() {
    let p = Level62Params {
        lambda: 0.02,
        vkink: 1.0e9,
        ..Level62Params::default()
    };
    let geom = unit_geom();
    let (vgs, vds) = (5.0, 8.0);

    let vth = KB * T_NOM_K / Q;
    let vsth = 7.0 * vth;
    let cox = 3.9 * E0 / 1.0e-7;
    let vmin = 2.0 * vsth;
    let r = vgs / vmin;
    let vgte = 0.5 * vmin * (1.0 + r + (16.0 + (r - 1.0).powi(2)).sqrt());
    let mufet = 1.0 / (1.0 / 1.0e-2 + 1.0 / (2.2e-7 * (2.0 * vgte / vsth).powi(3)));
    let vdsat = vgte;
    assert!(vds > vdsat, "test bias must exercise the saturation branch");

    let ia = mufet * cox * (vgte * vgte / 2.0) * (1.0 + p.lambda * vds);
    let isub = 1.0e-4 * cox * vsth * vsth * (vgs / vsth).exp() * (1.0 - (-vds / vsth).exp());
    let ileak = (150.0 * 100.0e-6 * (-0.68 / vth).exp() * (1.0 - (-vds / vth).exp())).max(0.0);
    let lo = ia.min(isub);
    let ratio = (lo / ia.max(isub)).sqrt();
    let expected = lo / (1.0 + ratio).powi(2) + ileak;

    let got = level62_current(&p, geom, T_NOM_K, vgs, vds);
    let rel = (got - expected).abs() / expected;
    assert!(
        rel < 1.0e-10,
        "saturation branch: got {got:e} vs {expected:e} (rel {rel:e})"
    );
}

#[test]
fn level62_current_stays_finite_at_extreme_overdrive() {
    // The published subthreshold exp(VGT/Vsth) overflows f64 to +inf at high gate overdrive
    // (VGT/Vsth past ~709), which made the current crossover NaN — Cadence/ngspice
    // broke at high |Vgs|. The capped exponent keeps Ids finite and positive far past any
    // physical bias. (ETA=7 default -> Vsth≈0.18 V, so Vgs=130 already overflows uncapped.)
    let p = Level62Params::default();
    for vgs in [20.0, 60.0, 130.0, 300.0] {
        let id = level62_current(&p, unit_geom(), T_NOM_K, vgs, 5.0);
        assert!(
            id.is_finite() && id > 0.0,
            "Ids must stay finite & positive at Vgs={vgs}, got {id:e}"
        );
    }
}

#[test]
fn level62_series_resistance_reduces_internal_channel_current() {
    let ideal = Level62Params {
        vto: 0.0,
        ..Level62Params::default()
    };
    let with_contacts = Level62Params {
        rs: 25_000.0,
        rd: 25_000.0,
        ..ideal
    };
    let geom = GeometryParams {
        w_um: 1_000.0,
        l_um: 5.0,
    };

    let id_ideal = level62_current(&ideal, geom, T_NOM_K, 10.0, 10.0);
    let id_series = level62_current(&with_contacts, geom, T_NOM_K, 10.0, 10.0);

    assert!(
        id_series.is_finite() && id_series > 0.0,
        "series-R current must stay finite and positive: {id_series:e}"
    );
    assert!(
        id_series < id_ideal * 0.75,
        "RS/RD should lower internal channel bias/current: ideal={id_ideal:e}, series={id_series:e}"
    );
}

/// The DIBL + temperature structure is an exact no-op at
/// the shipped defaults (`AT = BT = 0`, zero temperature coefficients). The inert
/// shape parameters (`VSI`/`VST`) and an off-`TNOM` evaluation must not perturb the
/// current while the strengths are zero, preserving fits created before those
/// parameters were added.
#[test]
fn level62_dibl_and_temperature_are_exact_noops_at_defaults() {
    let base = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    assert_eq!(
        (base.at, base.bt, base.dvto, base.dmu1, base.dasat, base.lasat),
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        "DIBL + temperature strengths ship off by default"
    );
    // Inert while AT = BT = 0: arbitrary VSI/VST values change nothing.
    let shuffled = Level62Params {
        vsi: 0.2,
        vst: 1.7,
        ..base
    };
    // Zero coefficients: an off-TNOM reference temperature changes nothing either.
    let retnom = Level62Params {
        tnom_k: 250.0,
        ..base
    };
    let geom = unit_geom();
    for vgs in [-2.0, 0.5, 1.0, 3.0, 8.0] {
        for vds in [0.05, 0.1, 1.0, 10.0, 40.0] {
            let want = level62_current(&base, geom, T_NOM_K, vgs, vds);
            assert_eq!(
                level62_current(&shuffled, geom, T_NOM_K, vgs, vds),
                want,
                "VSI/VST must be inert while AT = BT = 0 (vgs={vgs}, vds={vds})"
            );
            assert_eq!(
                level62_current(&retnom, geom, T_NOM_K, vgs, vds),
                want,
                "TNOM must be inert while all temperature coefficients are 0 (vgs={vgs}, vds={vds})"
            );
        }
    }
}

/// The DIBL term lowers the effective threshold with drain bias — a large subthreshold
/// current boost at high `Vds` — and fades above threshold as the `exp` gate-dependence
/// grows (manual p. 273 `VTeff`).
#[test]
fn level62_dibl_lowers_threshold_with_drain_bias_and_fades_above_threshold() {
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let off = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let on = Level62Params {
        at: 3.0e-8, // the manual's own defaults
        bt: 1.9e-6,
        ..off
    };
    // Subthreshold (Vgs well below VTO), saturation drain bias: DIBL boosts Id hard.
    let sub_on = level62_current(&on, geom, T_NOM_K, 0.5, 10.0);
    let sub_off = level62_current(&off, geom, T_NOM_K, 0.5, 10.0);
    assert!(
        sub_on > sub_off * 5.0,
        "DIBL lifts subthreshold current at high Vds: {sub_off:e} -> {sub_on:e}"
    );
    // The boost grows with Vds (the AT·Vds² numerator).
    let sub_on_lo = level62_current(&on, geom, T_NOM_K, 0.5, 1.0);
    let sub_off_lo = level62_current(&off, geom, T_NOM_K, 0.5, 1.0);
    assert!(
        sub_on / sub_off > sub_on_lo / sub_off_lo * 1.5,
        "DIBL grows with drain bias"
    );
    // Above threshold the exp() denominator suppresses the shift to a small correction.
    let onr_on = level62_current(&on, geom, T_NOM_K, 6.0, 10.0);
    let onr_off = level62_current(&off, geom, T_NOM_K, 6.0, 10.0);
    assert!(
        (onr_on - onr_off).abs() < 0.05 * onr_off,
        "DIBL fades above threshold: {onr_off:e} vs {onr_on:e}"
    );
}

/// The temperature coefficients engage only away from `TNOM`: `DVTO` lowers the
/// threshold (raising subthreshold current) at elevated temperature, and is a no-op at
/// `TEMP = TNOM` (manual p. 275 `VTX`).
#[test]
fn level62_dvto_shifts_threshold_only_off_tnom() {
    let base = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let warm_coeff = Level62Params {
        dvto: 5.0e-3, // V/K
        ..base
    };
    let geom = unit_geom();
    let (vgs, vds) = (0.5, 0.1);
    // At TEMP = TNOM the coefficient is invisible.
    assert_eq!(
        level62_current(&warm_coeff, geom, T_NOM_K, vgs, vds),
        level62_current(&base, geom, T_NOM_K, vgs, vds),
        "DVTO must be a no-op at TEMP = TNOM"
    );
    // 50 K above TNOM: VTX = VTO − 0.25 V, so the subthreshold current rises well
    // beyond the bare kT effect (compared against the coefficient-free model at the
    // same elevated temperature).
    let t_hot = T_NOM_K + 50.0;
    let hot_with = level62_current(&warm_coeff, geom, t_hot, vgs, vds);
    let hot_without = level62_current(&base, geom, t_hot, vgs, vds);
    assert!(
        hot_with > hot_without * 2.0,
        "DVTO lowers VT at elevated temperature: {hot_without:e} -> {hot_with:e}"
    );
}

/// `LASAT` shrinks the saturation parameter for short channels
/// (`αsat = ASAT − LASAT/Leff`), lowering the saturation current.
#[test]
fn level62_lasat_shrinks_saturation_for_short_channels() {
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let base = Level62Params {
        vto: 1.0,
        vkink: 1.0e9, // kink off so the comparison is the bare square-law
        ..Level62Params::ltps()
    };
    let short = Level62Params {
        lasat: 2.0e-6, // 2 µm on a 10 µm channel -> αsat 1.0 -> 0.8
        ..base
    };
    let id_base = level62_current(&base, geom, T_NOM_K, 6.0, 8.0);
    let id_short = level62_current(&short, geom, T_NOM_K, 6.0, 8.0);
    assert!(
        id_short < id_base * 0.9,
        "LASAT lowers the saturation current: {id_base:e} -> {id_short:e}"
    );
}
