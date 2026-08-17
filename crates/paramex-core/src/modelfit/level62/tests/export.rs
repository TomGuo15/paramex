use super::*;

#[test]
fn level62_exports_deferred_gui_constants_in_the_va_card() {
    let p = Level62Params {
        eps: 9.9,
        rs: 12.3,
        rd: 45.6,
        ..Level62Params::ltps()
    };
    let va = level62_verilog_a_card("dev", &p, unit_geom(), T_NOM_K, Polarity::NChannel);
    assert!(
        va.contains("parameter real EPS = 9.9000e0"),
        "EPS in Verilog-A:\n{va}"
    );
    assert!(
        va.contains("parameter real RS = 1.2300e1"),
        "RS in Verilog-A:\n{va}"
    );
    assert!(
        va.contains("parameter real RD = 4.5600e1"),
        "RD in Verilog-A:\n{va}"
    );
    // The DIBL + temperature knobs are emitted at their fitted (no-op)
    // defaults so a downstream user can override them per instance.
    for frag in [
        "parameter real AT = 0.0000e0",
        "parameter real BT = 0.0000e0",
        "parameter real VSI = 2.0000e0",
        "parameter real VST = 2.0000e0",
        "parameter real DVTO = 0.0000e0",
        "parameter real DMU1 = 0.0000e0",
        "parameter real DASAT = 0.0000e0",
        "parameter real LASAT = 0.0000e0",
    ] {
        assert!(va.contains(frag), "Verilog-A missing `{frag}`:\n{va}");
    }
}

/// The Verilog-A analog block mirrors `level62_current` line-for-line (forward↔.va
/// invariant): the rise-then-saturate mobility, per-regime Ia, stabilized channel,
/// the impact-ionization kink, and the leakage floor are all present — plus the
/// AC-capable Meyer gate-charge block (`ddt(Q)`, the same long-channel form as Level 61).
#[test]
fn level62_verilog_a_card_mirrors_the_forward_equations() {
    let va = level62_verilog_a_card(
        "dev",
        &Level62Params::ltps(),
        unit_geom(),
        T_NOM_K,
        Polarity::NChannel,
    );
    for frag in [
        "module ltps_l62(d, g, s)",
        "1.0 / (1.0 / MU0 + 1.0 / (mu1t * pow(2.0 * vgte / vsth, MMU)))", // rise-then-saturate µFET
        // Temperature structure: kT/q and the coefficients follow the
        // simulator's $temperature; dt = 0 at TEMP = TNOM reproduces the fit.
        "dt = $temperature - TNOM_K;",
        "vth = `P_K * $temperature / `P_Q;",
        "mu1t = MU1 + DMU1 * dt;",
        "asatt = max(ASAT - LASAT / l_m - DASAT * dt, 1.0e-6);",
        // n-channel-frame threshold with VTO folded by TYPE (device-frame VTO). The
        // buggy `vgsn - VTO` (device-frame VTO minus n-frame gate) is invisible for
        // n-channel but wrong by 2*VTO for p-channel — caught against Cadence Spectre.
        "vtx = TYPE * VTO - DVTO * dt;",
        // DIBL-shifted effective threshold, capped exponent like the
        // subthreshold branch.
        "vteff = vtx - (AT * vdsn * vdsn + BT) / (l_m * (1.0 + exp(min((vgsn - VST - vtx) / max(VSI, 1.0e-9), 80.0))));",
        "vgt = vgsn - vteff;",
        "(vdsn <= vdsat) ?",                                            // per-regime Ia
        ")) * (1.0 + LAMBDA * vdsn);",                                  // continuous CLM factor
        "ilo = min(ia, isub)",                                          // stable generalized harmonic blend
        "iratio = sqrt(ilo / max(ia, isub))",
        "ichan = ilo / ((1.0 + iratio) * (1.0 + iratio))",
        "w_m = (W < 1.0e-2) ? W : W * 1.0e-6;",                         // bare um or Cadence suffix
        "l_m = max((L < 1.0e-2) ? L : L * 1.0e-6, 1.0e-12);",
        "wl = w_m / l_m",
        "electrical di, si;",
        "I(d, di) <+ V(d, di) / RD;",
        "I(si, s) <+ V(si, s) / RS;",
        "pow(LKINK / l_m, MK)",                                        // the kink
        "I00 * w_m",                                                    // leakage width
        "exp(min(vgt / vsth, 80.0))",                                  // subthreshold exp overflow cap
        "(ichan + ileak) * (1.0 + ikink)",                             // total-current assembly
        "I(di, si) <+ TYPE * ids",
        // AC/transient Meyer gate charge (the same form as the Level 61 sibling).
        "area = w_m * l_m;",
        "qg = (2.0 / 3.0) * cox * area * (vgte * vgte + vgte * vgdt + vgdt * vgdt) / (vgte + vgdt + 1.0e-9)",
        "I(g, si) <+ ddt(TYPE * qg * (1.0 - fd))",
        "I(g, di) <+ ddt(TYPE * qg * fd)",
        // Small-signal operating-point outputs (gm/gds + gate-charge caps) so the model
        // shows it has AC in Spectre / ngspice and reads like the built-in psitft.
        "gm = ddx(TYPE * ids, V(g));",
        "cgg = ddx(TYPE * qg, V(g));",
        "cgs = cgg - cgd;",
    ] {
        assert!(va.contains(frag), "Verilog-A missing `{frag}`:\n{va}");
    }
    // Guard against regressing to the frame-mixing overdrive (n-frame gate minus
    // device-frame VTO) that broke every p-channel device's off-state/C-V in Spectre.
    assert!(
        !va.contains("vgt = vgsn - VTO;") && !va.contains("vtx = VTO"),
        "regressed to frame-mixed threshold (device-frame VTO in an n-frame equation):\n{va}"
    );
}

/// The p-channel Verilog-A folds polarity via `TYPE = −1` and flips `VTO`.
#[test]
fn level62_verilog_a_card_folds_pchannel_type() {
    let p = Level62Params {
        vto: 1.5,
        ..Level62Params::ltps()
    };
    let va = level62_verilog_a_card("p", &p, unit_geom(), T_NOM_K, Polarity::PChannel);
    assert!(
        va.contains("parameter real TYPE = -1.0000e0"),
        "TYPE=-1 for p-channel:\n{va}"
    );
    assert!(
        va.contains("parameter real VTO = -1.5000e0"),
        "VTO flipped:\n{va}"
    );
}
