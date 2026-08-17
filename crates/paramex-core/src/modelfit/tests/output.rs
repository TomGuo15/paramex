use super::extract_output;
use crate::modelfit::forward::{output_card_current, output_curve};
use crate::modelfit::{OutputCurve, OutputParams, Polarity, SubthresholdParams};
use crate::shared::numpy_compat::gradient;

/// The bare above-threshold output term (the OLD overlay formula): the exported
/// card's `iab` with no off-state blend, for comparison against the full card.
fn bare_above(g0: f64, gamma: f64, out: &OutputParams, vgt: f64, vd: f64) -> f64 {
    let g_ch = g0 * vgt.powf(1.0 + gamma);
    let vdsat = out.alpha_sat * vgt;
    let vdse = vd / (1.0 + (vd / vdsat).powf(out.m)).powf(1.0 / out.m);
    g_ch * vdse * (1.0 + out.lambda * vd)
}

// output_card_current reproduces the EXPORTED .va DC current (above + below +
// IOFF). Well above threshold it matches the bare above-threshold term; near
// threshold the tanh blend suppresses it BELOW the bare term (the divergence the
// old overlay ignored); deep off it floors at IOFF.

#[test]
fn output_card_current_matches_bare_above_threshold_well_above() {
    let out = OutputParams::card_defaults();
    let sub = SubthresholdParams::card_defaults(); // SS=0.3, IOFF=1e-12
    let (g0, gamma, vd) = (1.0e-5, 0.5, 15.0);
    let vgt = 6.0; // >> 2*SS = 0.6, so blend -> 1 and below -> negligible
    let card = output_card_current(g0, gamma, 0.0, &out, &sub, vgt, &[vd])[0];
    let bare = bare_above(g0, gamma, &out, vgt, vd);
    assert!(
        (card - bare).abs() / bare < 0.01,
        "well above threshold the card current {card} matches bare {bare}"
    );
}

#[test]
fn output_card_current_blends_near_threshold_without_a_jump() {
    let out = OutputParams::card_defaults();
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let (g0, gamma, vd) = (1.0e-5, 0.5, 2.0);
    let vgt = 0.5; // < 2*SS = 0.6: the blend weight is < 1
    let card = output_card_current(g0, gamma, 0.0, &out, &sub, vgt, &[vd])[0];
    let bare = bare_above(g0, gamma, &out, vgt, vd);
    assert!(card > sub.ioff, "near-threshold current stays above IOFF");
    assert!(
        (card - bare).abs() / bare < 0.1,
        "near-threshold blend stays continuous with the on branch: card={card}, bare={bare}"
    );
}

#[test]
fn output_card_current_floors_and_rises_monotonically() {
    let out = OutputParams::card_defaults();
    let sub = SubthresholdParams::card_defaults();
    let (g0, gamma) = (1.0e-5, 0.5);
    let vds: Vec<f64> = (0..=40).map(|i| i as f64 * 0.5).collect(); // 0..20 V
                                                                    // Deep off (well below threshold): tiny, positive, near the IOFF floor.
    let off = output_card_current(g0, gamma, 0.0, &out, &sub, -2.0, &vds);
    assert!(
        off.iter().all(|&i| i > 0.0),
        "off-state stays positive (IOFF)"
    );
    assert!(*off.last().unwrap() < 1.0e-6, "deep-off current is small");
    // On, fixed overdrive: Id rises (non-decreasing) with Vd into saturation.
    let on = output_card_current(g0, gamma, 0.0, &out, &sub, 6.0, &vds);
    assert!(
        on.windows(2).all(|w| w[1] >= w[0] - 1.0e-30),
        "Id is non-decreasing in Vd"
    );
}

#[test]
fn output_card_transconductance_has_no_blend_notch() {
    let out = OutputParams {
        alpha_sat: 0.647_792_869_374_921_4,
        lambda: 0.003_117_420_710_779_620_6,
        m: 3.331_342_748_415_719_4,
    };
    let sub = SubthresholdParams {
        ss_v_dec: 0.498_107_532_6,
        ioff: 5.674_54e-10,
    };
    let gamma = -0.078_283_711_1;
    let vgt: Vec<_> = (0..400).map(|i| -1.0 + 6.0 * i as f64 / 399.0).collect();
    let id: Vec<_> = vgt
        .iter()
        .map(|&v| output_card_current(1.8e-6, gamma, 0.0, &out, &sub, v, &[5.0])[0])
        .collect();
    let gm = gradient(&id, &vgt);
    let maximum = gm.iter().copied().fold(0.0_f64, f64::max);
    let mut running = 0.0_f64;
    let mut worst_drop = 0.0_f64;
    for value in gm.into_iter().filter(|value| *value >= 0.05 * maximum) {
        running = running.max(value);
        worst_drop = worst_drop.max((running - value) / running);
    }
    assert!(
        worst_drop < 0.02,
        "value-only subthreshold crossover creates a {:.1}% gm notch",
        100.0 * worst_drop
    );
}

// Round-trip: synthesize Id-Vd output curves at several Vg with KNOWN saturation
// parameters via the forward output model, then confirm the closed-form
// extraction recovers alpha_sat, lambda, and m from the curve shape.

fn synth_output(
    vt: f64,
    p: &OutputParams,
    g_ch: f64,
    vgs: &[f64],
    vd_max: f64,
    step: f64,
) -> Vec<OutputCurve> {
    let n = (vd_max / step).round() as usize;
    let vds: Vec<f64> = (0..=n).map(|i| i as f64 * step).collect();
    vgs.iter()
        .map(|&vg| {
            let id = output_curve(vt, p, g_ch, vg, &vds);
            OutputCurve {
                vg,
                vds: vds.clone(),
                id,
            }
        })
        .collect()
}

#[test]
fn output_extraction_round_trips_alpha_lambda_m() {
    let vt = 2.0;
    let truth = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    let g_ch = 1.0e-5;
    let vgs = [6.0, 9.0, 12.0]; // overdrive 4, 7, 10
    let curves = synth_output(vt, &truth, g_ch, &vgs, 20.0, 0.1);

    let fit = extract_output(&curves, vt, Polarity::NChannel).expect("output fit");

    assert!(
        (fit.alpha_sat - truth.alpha_sat).abs() < 0.05,
        "alpha_sat={} (truth 0.7)",
        fit.alpha_sat
    );
    // lambda (channel-length modulation) is the least-precise output param: the
    // smoothed Vd_eff approaches Vdsat only asymptotically, so the saturation
    // region stays slightly convex and biases the output-conductance slope. ~30%
    // is realistic for a closed-form extraction (the literature treats lambda as
    // approximate); alpha_sat and m recover much tighter.
    assert!(
        (fit.lambda - truth.lambda).abs() < 0.005,
        "lambda={} (truth 0.01)",
        fit.lambda
    );
    assert!((fit.m - truth.m).abs() < 0.6, "m={} (truth 2.5)", fit.m);
}

#[test]
fn output_extraction_returns_none_below_threshold_only() {
    // All curves at Vg <= Vt: no conduction, nothing to extract.
    let truth = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    let curves = synth_output(2.0, &truth, 1.0e-5, &[0.0, 1.0], 20.0, 0.1);
    assert!(extract_output(&curves, 2.0, Polarity::NChannel).is_none());
}

#[test]
fn p_channel_output_round_trips() {
    // A p-channel device (VT = -2): mirror n-channel output curves through the
    // origin (Vg -> -Vg, Vds -> -Vds, |Id| unchanged). The polarity-aware
    // extraction must recover the SAME shape params (alpha_sat/lambda/m are
    // polarity-invariant).
    let vt_n = 2.0;
    let truth = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    let n_curves = synth_output(vt_n, &truth, 1.0e-5, &[6.0, 9.0, 12.0], 20.0, 0.1);
    let p_curves: Vec<OutputCurve> = n_curves
        .iter()
        .map(|c| OutputCurve {
            vg: -c.vg,
            vds: c.vds.iter().map(|v| -v).collect(),
            id: c.id.clone(),
        })
        .collect();

    let fit = extract_output(&p_curves, -vt_n, Polarity::PChannel).expect("p-channel output fit");
    assert!(
        (fit.alpha_sat - 0.7).abs() < 0.05,
        "alpha_sat={}",
        fit.alpha_sat
    );
    assert!((fit.lambda - 0.01).abs() < 0.005, "lambda={}", fit.lambda);
    assert!((fit.m - 2.5).abs() < 0.6, "m={}", fit.m);
    // The n-channel extraction must reject the p-channel curves (sanity: polarity matters).
    assert!(
        extract_output(&p_curves, -vt_n, Polarity::NChannel).is_none(),
        "n-channel extraction must not fit p-channel curves"
    );
}

#[test]
fn p_channel_output_extraction_handles_parser_ascending_vds() {
    // The output-curve parser (`sorted_curve`) hands each sub-sweep with Vds sorted
    // ASCENDING, so a p-channel device arrives with ascending *negative* Vds (e.g.
    // [-20..0]). extract_output negates that into the on-direction, which then runs
    // DESCENDING through extract_one_output -- exercising the interp() ascending
    // precondition that `p_channel_output_round_trips` (descending input) never hits.
    let vt_n = 2.0;
    let truth = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    let n_curves = synth_output(vt_n, &truth, 1.0e-5, &[6.0, 9.0, 12.0], 20.0, 0.1);
    // Mirror through the origin but present Vds ASCENDING (negative) like the parser:
    // reverse the negated ascending n-channel Vds and its Id together.
    let p_curves: Vec<OutputCurve> = n_curves
        .iter()
        .map(|c| OutputCurve {
            vg: -c.vg,
            vds: c.vds.iter().rev().map(|v| -v).collect(),
            id: c.id.iter().rev().copied().collect(),
        })
        .collect();
    for c in &p_curves {
        assert!(
            c.vds.windows(2).all(|w| w[0] <= w[1]),
            "parser contract: Vds ascending"
        );
    }

    let fit = extract_output(&p_curves, -vt_n, Polarity::PChannel).expect("p-channel output fit");
    assert!(
        (fit.alpha_sat - 0.7).abs() < 0.05,
        "alpha_sat={}",
        fit.alpha_sat
    );
    assert!((fit.lambda - 0.01).abs() < 0.005, "lambda={}", fit.lambda);
    assert!((fit.m - 2.5).abs() < 0.6, "m={}", fit.m);
}
