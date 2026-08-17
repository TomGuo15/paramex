//! Verification of the AC gate-charge model emitted into the `.va`. The charge
//! math lives in the Verilog-A text (export.rs), so this test MIRRORS it exactly
//! and pins it against the *verified physical limits* of the long-channel /
//! Meyer model — correctness comes from the endpoints, not from trusting the
//! partition algebra: gate-channel capacitance Cgg = Cox·W·L in deep triode and
//! (2/3)·Cox·W·L in saturation; gate-drain capacitance Cgd -> 0 in saturation
//! (≈ Cgs in triode); charge conservation (qgs + qgd == qg); and caps vanishing
//! below threshold. The sibling export test asserts the `.va` emits these exact
//! expressions.

/// Drain-overdrive smoothing — must match `VGTE_SMOOTH_V` in the core export.
const DLT: f64 = 5.0e-2;
/// Gate-CHARGE turn-on width — must match `CHARGE_SMOOTH_V` (wider than DLT so
/// C_gg co-locates with g_m; AC-only, zero DC effect).
const DLQ: f64 = 0.2;

fn smooth(x: f64, d: f64) -> f64 {
    0.5 * (x + (x * x + d * d).sqrt())
}

/// Mirror of the `.va` gate-charge block: returns `(qgs, qgd, qg)` in coulombs.
/// `cox` in F/m², `w_um`/`l_um` in µm, `vgt = Vgs - VT`, `vd = Vds` in volts.
fn gate_charges(cox: f64, w_um: f64, l_um: f64, vgt: f64, vd: f64) -> (f64, f64, f64) {
    let vgte = smooth(vgt, DLQ);
    let area = (w_um * 1.0e-6) * (l_um * 1.0e-6);
    let vgdt = smooth(vgte - vd, DLT);
    let qg = (2.0 / 3.0) * cox * area * (vgte * vgte + vgte * vgdt + vgdt * vgdt)
        / (vgte + vgdt + 1.0e-9);
    let fd = vgdt * vgdt / (vgte * vgte + vgdt * vgdt + 1.0e-18);
    (qg * (1.0 - fd), qg * fd, qg)
}

const COX: f64 = 1.0e-3; // F/m^2
const W: f64 = 100.0; // um
const L: f64 = 10.0; // um

/// Cox·W·L in farads — the full gate-channel capacitance.
fn cox_wl() -> f64 {
    COX * (W * 1.0e-6) * (L * 1.0e-6)
}

/// Numerical d(qg)/d(Vgt) at fixed Vds (central difference).
fn cgg(vgt: f64, vd: f64) -> f64 {
    let h = 1.0e-5;
    let (_, _, qp) = gate_charges(COX, W, L, vgt + h, vd);
    let (_, _, qm) = gate_charges(COX, W, L, vgt - h, vd);
    (qp - qm) / (2.0 * h)
}

/// Numerical gate-drain capacitance Cgd = -d(qgd)/d(Vds) at fixed Vgt.
fn cgd(vgt: f64, vd: f64) -> f64 {
    let h = 1.0e-5;
    let (_, qdp, _) = gate_charges(COX, W, L, vgt, vd + h);
    let (_, qdm, _) = gate_charges(COX, W, L, vgt, vd - h);
    -(qdp - qdm) / (2.0 * h)
}

#[test]
fn gate_charge_is_conserved() {
    for &(vgt, vd) in &[(2.0, 0.1), (2.0, 5.0), (0.5, 1.0), (-2.0, 3.0)] {
        let (qgs, qgd, qg) = gate_charges(COX, W, L, vgt, vd);
        assert!(
            (qgs + qgd - qg).abs() <= qg.abs() * 1.0e-12 + 1.0e-30,
            "qgs+qgd must equal qg at (vgt={vgt}, vd={vd}): {qgs}+{qgd} != {qg}"
        );
    }
}

#[test]
fn cgg_hits_the_triode_and_saturation_limits() {
    let full = cox_wl();
    // Deep triode (Vds -> 0): Cgg -> Cox*W*L.
    let triode = cgg(2.0, 1.0e-3);
    assert!(
        (triode / full - 1.0).abs() < 0.02,
        "triode Cgg {triode} should be ~Cox*W*L {full}"
    );
    // Saturation (Vds >> Vgt): Cgg -> (2/3)*Cox*W*L.
    let sat = cgg(2.0, 6.0);
    assert!(
        (sat / (full * 2.0 / 3.0) - 1.0).abs() < 0.02,
        "saturation Cgg {sat} should be ~(2/3)Cox*W*L {}",
        full * 2.0 / 3.0
    );
}

#[test]
fn cgd_vanishes_in_saturation_and_is_significant_in_triode() {
    let triode = cgd(2.0, 1.0e-3);
    let sat = cgd(2.0, 6.0);
    assert!(
        triode > 0.2 * cox_wl(),
        "triode Cgd should be sizeable: {triode}"
    );
    assert!(
        sat.abs() < triode * 1.0e-2,
        "saturation Cgd {sat} should collapse vs triode {triode}"
    );
}

#[test]
fn caps_vanish_below_threshold() {
    let full = cox_wl();
    // Well below threshold the smooth overdrive -> 0, so the gate charge and its
    // capacitance are negligible (no channel to charge).
    let off = cgg(-5.0, 1.0);
    assert!(
        off < 1.0e-3 * full,
        "below-threshold Cgg {off} should be ~0 vs {full}"
    );
}

#[test]
fn cox_zero_gives_no_charge() {
    // COX = 0 -> DC-only fallback: every gate charge is exactly zero.
    let (qgs, qgd, qg) = gate_charges(0.0, W, L, 2.0, 1.0);
    assert_eq!((qgs, qgd, qg), (0.0, 0.0, 0.0));
}
