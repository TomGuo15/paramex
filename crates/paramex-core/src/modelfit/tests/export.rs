use super::verilog_a_card;
use crate::modelfit::{
    AboveThresholdFit, BiasParams, GeometryParams, OutputParams, Polarity, SubthresholdParams,
};

// The Verilog-A module IS the model (strict Eq.25 + real off state + a charge-based gate
// model for AC) — the Cadence-Spectre / ADS / ngspice-usable form of these custom compact
// models, `ahdl_include`d and instantiated directly. A deterministic text projection of the
// extracted params, pinned so a format change is caught.

fn fit() -> AboveThresholdFit {
    AboveThresholdFit {
        vt: 2.02,
        gamma: 0.492,
        k: 1.021e-6,
        r2: 0.999,
    }
}

fn geom() -> GeometryParams {
    GeometryParams::default()
}

fn bias() -> BiasParams {
    BiasParams::default()
}

#[test]
fn verilog_a_card_is_p_channel_when_polarity_is_pchannel() {
    // A p-channel device folds polarity via TYPE=-1 with the device-frame VTO.
    let pfit = AboveThresholdFit {
        vt: -3.0,
        gamma: 0.5,
        k: 1.0e-6,
        r2: 0.99,
    };
    let va = verilog_a_card(
        "pdev",
        &pfit,
        None,
        None,
        geom(),
        bias(),
        Polarity::PChannel,
    );
    assert!(
        va.contains("parameter real TYPE = -1.0000e0"),
        "p-channel TYPE; va:\n{va}"
    );
    assert!(
        va.contains("parameter real VTO = -3.0000e0"),
        "negative VTO; va:\n{va}"
    );
}

#[test]
fn verilog_a_card_is_strict_eq25_polarity_aware_with_a_real_off_state() {
    let output = OutputParams {
        alpha_sat: 0.704,
        lambda: 0.0102,
        m: 2.51,
    };
    let sub = SubthresholdParams {
        ss_v_dec: 0.28,
        ioff: 5.0e-13,
    };
    let va = verilog_a_card(
        "demo: organic",
        &fit(),
        Some(&output),
        Some(&sub),
        geom(),
        bias(),
        Polarity::NChannel,
    );

    assert!(va.contains("`include \"disciplines.vams\""), "va:\n{va}");
    assert!(va.contains("module aostft(d, g, s);"), "va:\n{va}");
    assert!(
        va.contains("parameter real TYPE = 1.0000e0"),
        "n-channel TYPE; va:\n{va}"
    );
    assert!(va.contains("parameter real VTO = 2.0200e0"), "va:\n{va}");
    assert!(va.contains("parameter real GAMMA = 4.9200e-1"), "va:\n{va}");
    assert!(va.contains("parameter real KP = 3.4033e-7"), "va:\n{va}");
    assert!(va.contains("parameter real W = 1.5000e3"), "va:\n{va}");
    assert!(va.contains("parameter real L = 5.0000e1"), "va:\n{va}");
    assert!(va.contains("parameter real RS = 0.0000e0"), "va:\n{va}");
    assert!(
        va.contains("parameter real ALPHASAT = 7.0400e-1"),
        "va:\n{va}"
    );
    // Polarity folded in: TYPE on the overdrive, drain, and output current.
    assert!(
        va.contains("vgt = TYPE * (V(g, s) - VTO);"),
        "TYPE overdrive; va:\n{va}"
    );
    assert!(va.contains("vd = TYPE * V(d, s);"), "TYPE drain; va:\n{va}");
    assert!(
        va.contains("I(d, s) <+ TYPE * ids;"),
        "TYPE current; va:\n{va}"
    );
    // Strict Eq.25: smooth overdrive, channel CONDUCTANCE, un-normalized Vdse.
    assert!(
        va.contains("vgte = 0.5 * (vgt + sqrt(vgt * vgt + dlt * dlt))"),
        "smooth overdrive; va:\n{va}"
    );
    assert!(
        va.contains("gch = wl * KP * pow(vgte, 1.0 + GAMMA)"),
        "conductance gain; va:\n{va}"
    );
    assert!(
        va.contains("iab = gch * vdse * (1.0 + LAMBDA * vd)"),
        "strict drain (no /vdsat); va:\n{va}"
    );
    assert!(
        va.contains("dv = sqrt(max(((2.0 + GAMMA) * SS / ln(10.0))"),
        "derivative-matched crossover; va:\n{va}"
    );
    assert!(
        !va.contains("vdse / vdsat"),
        "must not normalize by vdsat; va:\n{va}"
    );
    // Real off state (Eq.28/29) + IOFF floor.
    assert!(
        va.contains("ids0 = iabc * exp("),
        "subthreshold anchor; va:\n{va}"
    );
    assert!(
        va.contains("below = ids0 * exp(ln(10.0) * vgt / SS"),
        "exponential subthreshold; va:\n{va}"
    );
    // 'above' is a reserved Verilog-AMS analog operator; the temp must be `ids_above`
    // (a bare `above` variable fails Spectre compile with VACOMP-2175 — caught on the server).
    assert!(
        va.contains("ids = ids_above + below + IOFF;"),
        "blend + floor; va:\n{va}"
    );
    assert!(
        va.contains("ids_above = iab /"),
        "uses ids_above (not reserved `above`); va:\n{va}"
    );
    assert!(
        !va.contains(" above ="),
        "must not assign a reserved `above` variable; va:\n{va}"
    );
    assert!(!va.contains("ids = 0.0;"), "no hard-zero off; va:\n{va}");
    // Geometry accepts either bare micrometer numbers (W=1500) or Cadence SI suffix
    // values (W=1500u), then uses meters everywhere internally.
    for frag in [
        "w_m = (W < 1.0e-2) ? W : W * 1.0e-6;",
        "l_m = max((L < 1.0e-2) ? L : L * 1.0e-6, 1.0e-12);",
        "wl = w_m / l_m;",
    ] {
        assert!(va.contains(frag), "geometry normalization `{frag}`:\n{va}");
    }
    // AC charge model (Cox*W*L) contributed via ddt(), folded by TYPE.
    assert!(va.contains("parameter real COX ="), "Cox param; va:\n{va}");
    assert!(va.contains("area = w_m * l_m;"), "gate area; va:\n{va}");
    assert!(
        va.contains("qg = (2.0 / 3.0) * COX * area *"),
        "gate-channel charge; va:\n{va}"
    );
    assert!(
        va.contains("I(g, s) <+ ddt(TYPE * qg * (1.0 - fd));"),
        "g-s charge; va:\n{va}"
    );
    assert!(
        va.contains("I(g, d) <+ ddt(TYPE * qg * fd);"),
        "g-d charge; va:\n{va}"
    );
    assert!(va.contains("endmodule"), "va:\n{va}");
}

#[test]
fn verilog_a_card_exposes_small_signal_operating_point() {
    // Without these, Spectre's Results Display shows only the module's internal reals and
    // the model "looks DC-only". gm/gds (I-V slopes) + Cgg/Cgs/Cgd (gate-charge derivatives)
    // make it read like a built-in device and visibly carry AC. Validated in ngspice OSDI:
    // gm/gds finite, Cgg matches the AC-measured value, Cgs+Cgd=Cgg (charge-conserving).
    let va = verilog_a_card("d", &fit(), None, None, geom(), bias(), Polarity::NChannel);
    for frag in [
        "(* desc=\"DC transconductance dId/dVgs\", units=\"S\" *) real gm;",
        "(* desc=\"gate capacitance Cgg=dQg/dVg\", units=\"F\" *) real cgg;",
        "gm = ddx(TYPE * ids, V(g));",
        "gds = ddx(TYPE * ids, V(d));",
        "cgg = ddx(TYPE * qg, V(g));",
        "cgd = -ddx(TYPE * qg, V(d));",
        "cgs = cgg - cgd;",
    ] {
        assert!(va.contains(frag), "va missing op-point `{frag}`:\n{va}");
    }
}

#[test]
fn verilog_a_card_defaults_params_without_data() {
    let va = verilog_a_card("d", &fit(), None, None, geom(), bias(), Polarity::NChannel);
    assert!(
        va.contains("parameter real ALPHASAT = 6.0000e-1"),
        "va:\n{va}"
    );
    assert!(va.contains("parameter real MSAT = 2.5000e0"), "va:\n{va}");
    assert!(va.contains("parameter real SS = 3.0000e-1"), "va:\n{va}");
    assert!(va.contains("parameter real IOFF = 1.0000e-12"), "va:\n{va}");
    assert!(va.contains("parameter real W = 1.5000e3"), "va:\n{va}");
    assert!(va.contains("parameter real L = 5.0000e1"), "va:\n{va}");
    assert!(va.contains("parameter real RS = 0.0000e0"), "va:\n{va}");
}

#[test]
fn nonphysical_extracted_subthreshold_falls_back_to_a_valid_card() {
    // A noisy / wrong-polarity off-region can make extraction hand back a negative
    // (or zero / non-finite) SS or IOFF. Emitting it raw would write `SS = -2…`
    // into the card and blow up its exp(2.3*(Vgs-VTO)/SS) in Cadence. The export
    // must fall back to the provisional defaults so every card stays a valid,
    // simulable Verilog-A.
    let bad = SubthresholdParams {
        ss_v_dec: -2.0,
        ioff: -1.0e-9,
    };
    let va = verilog_a_card(
        "d",
        &fit(),
        None,
        Some(&bad),
        geom(),
        bias(),
        Polarity::NChannel,
    );
    assert!(!va.contains("SS = -"), "no negative SS in the card:\n{va}");
    assert!(
        !va.contains("IOFF = -"),
        "no negative IOFF in the card:\n{va}"
    );
    assert!(
        va.contains("parameter real SS = 3.0000e-1"),
        "SS falls back to 0.3:\n{va}"
    );
    assert!(
        va.contains("parameter real IOFF = 1.0000e-12"),
        "IOFF falls back to 1e-12:\n{va}"
    );
}
