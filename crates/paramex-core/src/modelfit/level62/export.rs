use super::params::Level62Params;
use crate::modelfit::export::sanitize_model_name;
use crate::modelfit::types::{GeometryParams, Polarity};

/// A self-contained Verilog-A module of the **Level 62-derived, analog-stabilized** DC model
/// with this device's parameters as defaults — the portable Spectre / ngspice-OSDI artifact.
/// The analog block mirrors [`super::forward::level62_current`] line-for-line (the forward↔`.va`
/// invariant, with `$temperature` standing in for `temp_k`): the DIBL-shifted `VTeff`,
/// rise-then-saturate mobility, the per-regime square-law `Ia`, the
/// order-1/2 generalized-harmonic current channel, the reverse-diode leakage floor, and the
/// impact-ionization kink — all in the n-channel frame with a `TYPE = ±1` polarity fold.
/// Temperature-capable: `kT/q` and the `DVTO`/`DMU1`/`DASAT` coefficients follow the
/// simulator's `$temperature` against `TNOM_K` (all coefficients default 0, and
/// `TEMP = TNOM` reproduces the fit exactly). AC-capable: a charge-based Meyer gate
/// model (the same long-channel form as the Level 61 sibling) adds real terminal
/// capacitances via `ddt(Q)` (zero in DC, so the DC solution is unchanged) — see
/// Uses `constants.vams` for `P_Q`, `P_K`, `P_EPS0`.
pub(in crate::modelfit) fn level62_verilog_a_card(
    device_name: &str,
    params: &Level62Params,
    geom: GeometryParams,
    temp_k: f64,
    polarity: Polarity,
) -> String {
    use std::fmt::Write as _;
    let model = sanitize_model_name(device_name);
    let p = params;
    let vto = polarity.map_vg(p.vto);
    let vfb = polarity.map_vg(p.vfb);
    let ty = polarity.sign();
    let mut va = String::new();
    let _ = write!(
        va,
        "// ParamEx Model Fit \u{2014} Level 62-derived LTPS, analog-stabilized (Verilog-A) for device: {device_name}\n\
         // NOT canonical HSPICE Level 62: ParamEx replaces only the moderate-inversion\n\
         // crossover; the surrounding branch equations remain Level 62-derived.\n\
         // DC analog block mirrors ParamEx's Level 62 forward equation:\n\
         // DIBL-shifted VTeff, rise-then-saturate mobility, stabilized current crossover,\n\
         // reverse-diode leakage, and the impact-ionization kink (1+Ikink). Temperature follows\n\
         // $temperature vs TNOM_K (fit was at TEMP = TNOM; DVTO/DMU1/DASAT default 0). DIBL is\n\
         // fitted-off (AT = BT = 0) unless overridden; AT/BT/VSI/VST/DVTO are n-channel-frame\n\
         // parameters applied after the TYPE fold. Spectre:  ahdl_include \"{model}.va\";  M0 (d g s) ltps_l62\n\
         `include \"constants.vams\"\n\
         `include \"disciplines.vams\"\n\
         \n\
         module ltps_l62(d, g, s);\n\
         \x20\x20inout d, g, s;\n\
         \x20\x20electrical d, g, s;\n\
         \x20\x20electrical di, si;\n\
         \x20\x20parameter real TYPE = {ty:.4e};\x20\x20// +1 n-channel, -1 p-channel\n\
         \x20\x20parameter real VTO = {vto:.4e};\n\
         \x20\x20parameter real VFB = {vfb:.4e};\n\
         \x20\x20parameter real MU0 = {mu0:.4e};\x20\x20// m^2/Vs\n\
         \x20\x20parameter real MU1 = {mu1:.4e};\x20\x20// m^2/Vs\n\
         \x20\x20parameter real MMU = {mmu:.4e};\n\
         \x20\x20parameter real MUS = {mus:.4e};\x20\x20// m^2/Vs\n\
         \x20\x20parameter real ASAT = {asat:.4e};\n\
         \x20\x20parameter real LAMBDA = {lambda:.4e};\n\
         \x20\x20parameter real DELTA = {delta:.4e};\n\
         \x20\x20parameter real ETA = {eta:.4e};\n\
         \x20\x20parameter real VKINK = {vkink:.4e};\n\
         \x20\x20parameter real LKINK = {lkink:.4e};\x20\x20// m\n\
         \x20\x20parameter real MK = {mk:.4e};\n\
         \x20\x20parameter real I00 = {i00:.4e};\x20\x20// A/m\n\
         \x20\x20parameter real EB = {eb:.4e};\x20\x20// eV\n\
         \x20\x20parameter real EPS = {eps:.4e};\x20\x20// reserved for deferred leakage field terms\n\
         \x20\x20parameter real EPSI = {epsi:.4e};\n\
         \x20\x20parameter real TOX = {tox:.4e};\x20\x20// m\n\
         \x20\x20parameter real RS = {rs:.4e};\x20\x20// Ohm\n\
         \x20\x20parameter real RD = {rd:.4e};\x20\x20// Ohm\n\
         \x20\x20parameter real AT = {at:.4e};\x20\x20// m/V, DIBL strength (0 = off; manual default 3e-8)\n\
         \x20\x20parameter real BT = {bt:.4e};\x20\x20// m*V, DIBL offset (0 = off; manual default 1.9e-6)\n\
         \x20\x20parameter real VSI = {vsi:.4e};\x20\x20// V, DIBL gate-dependence width\n\
         \x20\x20parameter real VST = {vst:.4e};\x20\x20// V, DIBL gate-dependence offset\n\
         \x20\x20parameter real DVTO = {dvto:.4e};\x20\x20// V/K, VTO temperature coefficient\n\
         \x20\x20parameter real DMU1 = {dmu1:.4e};\x20\x20// m^2/Vs/K, MU1 temperature coefficient\n\
         \x20\x20parameter real DASAT = {dasat:.4e};\x20\x20// 1/K, ASAT temperature coefficient\n\
         \x20\x20parameter real LASAT = {lasat:.4e};\x20\x20// m, ASAT length dependence\n\
         \x20\x20parameter real W = {w:.4e};\x20\x20// bare um number or SI value, e.g. 1500 or 1500u\n\
         \x20\x20parameter real L = {l:.4e};\x20\x20// bare um number or SI value, e.g. 50 or 50u\n\
         \x20\x20parameter real TNOM_K = {tnom_k:.4e};\x20\x20// measurement temperature (K)\n\
         \x20\x20real vgsn, vdsn, vgt, vth, vsth, cox, w_m, l_m, wl, vmin, vgte, mufet, vdsat;\n\
         \x20\x20real dt, vtx, vteff, mu1t, asatt;\x20\x20// DIBL + temperature structure\n\
         \x20\x20real iacore, ia, isub, ilo, iratio, ichan, vdsk, excess, ikink, ileak, ids;\n\
         \x20\x20real area, qdlt, vgdt, qg, fd;\x20\x20// AC gate-charge model\n\
         \x20\x20// Operating-point outputs (Spectre Results Display / ngspice OSDI): the standard\n\
         \x20\x20// small-signal set, so the model reads like a real device (and shows it has AC).\n\
         \x20\x20(* desc=\"DC transconductance dId/dVgs\", units=\"S\" *) real gm;\n\
         \x20\x20(* desc=\"DC output conductance dId/dVds\", units=\"S\" *) real gds;\n\
         \x20\x20(* desc=\"gate capacitance Cgg=dQg/dVg\", units=\"F\" *) real cgg;\n\
         \x20\x20(* desc=\"gate-drain capacitance\", units=\"F\" *) real cgd;\n\
         \x20\x20(* desc=\"gate-source capacitance\", units=\"F\" *) real cgs;\n\
         \x20\x20analog begin\n\
         \x20\x20\x20\x20// External contacts feed the intrinsic TFT through RS/RD; zero means ideal short.\n\
         \x20\x20\x20\x20if (RD > 0.0) begin\n\
         \x20\x20\x20\x20\x20\x20I(d, di) <+ V(d, di) / RD;\n\
         \x20\x20\x20\x20end else begin\n\
         \x20\x20\x20\x20\x20\x20V(d, di) <+ 0.0;\n\
         \x20\x20\x20\x20end\n\
         \x20\x20\x20\x20if (RS > 0.0) begin\n\
         \x20\x20\x20\x20\x20\x20I(si, s) <+ V(si, s) / RS;\n\
         \x20\x20\x20\x20end else begin\n\
         \x20\x20\x20\x20\x20\x20V(si, s) <+ 0.0;\n\
         \x20\x20\x20\x20end\n\
         \x20\x20\x20\x20// n-channel-on frame (TYPE folds polarity), then send current out with TYPE.\n\
         \x20\x20\x20\x20vgsn = TYPE * V(g, si);\n\
         \x20\x20\x20\x20vdsn = TYPE * V(di, si);\n\
         \x20\x20\x20\x20// Temperature follows the simulator ($temperature, K); dt = 0 at TEMP = TNOM\n\
         \x20\x20\x20\x20// reproduces the fitted curve exactly (fit was isothermal at TNOM_K).\n\
         \x20\x20\x20\x20dt = $temperature - TNOM_K;\n\
         \x20\x20\x20\x20vth = `P_K * $temperature / `P_Q;\n\
         \x20\x20\x20\x20vsth = max(ETA * vth, 1.0e-9);\n\
         \x20\x20\x20\x20cox = EPSI * `P_EPS0 / TOX;\n\
         \x20\x20\x20\x20// ponytail: accepts bare um or Cadence suffix values; add explicit unit param if sub-0.01um devices matter.\n\
         \x20\x20\x20\x20w_m = (W < 1.0e-2) ? W : W * 1.0e-6;\n\
         \x20\x20\x20\x20l_m = max((L < 1.0e-2) ? L : L * 1.0e-6, 1.0e-12);\n\
         \x20\x20\x20\x20wl = w_m / l_m;\n\
         \x20\x20\x20\x20// n-channel-frame threshold: VTO is emitted in the DEVICE frame, so fold it\n\
         \x20\x20\x20\x20// with TYPE; DVTO is an n-frame coefficient applied after the fold.\n\
         \x20\x20\x20\x20vtx = TYPE * VTO - DVTO * dt;\n\
         \x20\x20\x20\x20// DIBL-shifted effective threshold (HSPICE manual p.273):\n\
         \x20\x20\x20\x20// VTeff = VTX - (AT*Vds^2 + BT)/(Leff*(1 + exp((Vgs - VST - VTX)/VSI))).\n\
         \x20\x20\x20\x20// Gate exponent capped at 80 so exp() cannot overflow (shift is ~0 there);\n\
         \x20\x20\x20\x20// AT = BT = 0 (the fitted default) makes VTeff = VTX exactly.\n\
         \x20\x20\x20\x20vteff = vtx - (AT * vdsn * vdsn + BT) / (l_m * (1.0 + exp(min((vgsn - VST - vtx) / max(VSI, 1.0e-9), 80.0))));\n\
         \x20\x20\x20\x20// n-channel-on overdrive from the effective threshold (matches level62_current).\n\
         \x20\x20\x20\x20vgt = vgsn - vteff;\n\
         \x20\x20\x20\x20vmin = 2.0 * vsth;\n\
         \x20\x20\x20\x20// RPI smooth-clamped effective overdrive (C-inf, always positive).\n\
         \x20\x20\x20\x20vgte = 0.5 * vmin * (1.0 + vgt / vmin + sqrt(DELTA * DELTA + (vgt / vmin - 1.0) * (vgt / vmin - 1.0)));\n\
         \x20\x20\x20\x20// rise-then-saturate poly-Si field-effect mobility (-> MU0 at large overdrive),\n\
         \x20\x20\x20\x20// with the MU1 temperature shift (no-op at dt = 0)\n\
         \x20\x20\x20\x20mu1t = MU1 + DMU1 * dt;\n\
         \x20\x20\x20\x20mufet = 1.0 / (1.0 / MU0 + 1.0 / (mu1t * pow(2.0 * vgte / vsth, MMU)));\n\
         \x20\x20\x20\x20// saturation parameter with its length + temperature terms (= ASAT at defaults)\n\
         \x20\x20\x20\x20asatt = max(ASAT - LASAT / l_m - DASAT * dt, 1.0e-6);\n\
         \x20\x20\x20\x20vdsat = max(asatt * vgte, 1.0e-12);\n\
         \x20\x20\x20\x20// above-threshold square-law, HSPICE per-regime form (switch at the knee)\n\
         \x20\x20\x20\x20// One CLM factor across both regions keeps Id and gds continuous at VDSAT.\n\
         \x20\x20\x20\x20iacore = ((vdsn <= vdsat) ? (vgte * vdsn - vdsn * vdsn / (2.0 * asatt)) : (asatt * vgte * vgte / 2.0)) * (1.0 + LAMBDA * vdsn);\n\
         \x20\x20\x20\x20ia = max(mufet * cox * wl * iacore, 1.0e-30);\n\
         \x20\x20\x20\x20// exponential subthreshold (diffusion) current\n\
         \x20\x20\x20\x20// gate exponent capped at 80 so exp() can't overflow to inf -> NaN in the\n\
         \x20\x20\x20\x20// stabilized blend (Isub >> Ia there, so Ichan -> Ia regardless); mirrors level62_current.\n\
         \x20\x20\x20\x20isub = max(MUS * cox * wl * vsth * vsth * exp(min(vgt / vsth, 80.0)) * (1.0 - exp(-vdsn / vsth)), 1.0e-30);\n\
         \x20\x20\x20\x20ilo = min(ia, isub);\n\
         \x20\x20\x20\x20iratio = sqrt(ilo / max(ia, isub));\n\
         \x20\x20\x20\x20ichan = ilo / ((1.0 + iratio) * (1.0 + iratio));\x20\x20// order-1/2 blend\n\
         \x20\x20\x20\x20// impact-ionization kink (only past pinch-off)\n\
         \x20\x20\x20\x20vdsk = vdsn / pow(1.0 + pow(vdsn / vdsat, 3.0), 1.0 / 3.0) - vth;\n\
         \x20\x20\x20\x20excess = vdsn - vdsk;\n\
         \x20\x20\x20\x20ikink = (excess > 1.0e-9) ? ((1.0 / VKINK) * pow(LKINK / l_m, MK) * excess * exp(-VKINK / excess)) : 0.0;\n\
         \x20\x20\x20\x20// reverse-diode off-state floor\n\
         \x20\x20\x20\x20ileak = max(I00 * w_m * exp(-EB / vth) * (1.0 - exp(-vdsn / vth)), 0.0);\n\
         \x20\x20\x20\x20ids = (ichan + ileak) * (1.0 + ikink);\n\
         \x20\x20\x20\x20I(di, si) <+ TYPE * ids;\n\
         \x20\x20\x20\x20// AC/transient gate-charge model: charge-based ddt(Q), the same long-channel\n\
         \x20\x20\x20\x20// Meyer gate charge as the Level 61 sibling (Cox internal from EPSI/TOX; the\n\
         \x20\x20\x20\x20// charge depends only on the overdrive, not the mobility/kink). qg gives\n\
         \x20\x20\x20\x20// Cgg -> Cox*W*L (triode) and (2/3)*Cox*W*L (saturation) once Vgt >> VMIN; the\n\
         \x20\x20\x20\x20// drain split fd gives Cgd -> 0 in saturation. ddt() is zero in DC, so the DC\n\
         \x20\x20\x20\x20// solution is unchanged. (cox = EPSI*eps0/TOX is already computed above.)\n\
         \x20\x20\x20\x20area = w_m * l_m;\x20\x20// m^2 for Cox(F/m^2)\n\
         \x20\x20\x20\x20qdlt = 5.0e-2;\x20\x20// drain-overdrive smoothing (V)\n\
         \x20\x20\x20\x20vgdt = 0.5 * ((vgte - vdsn) + sqrt((vgte - vdsn) * (vgte - vdsn) + qdlt * qdlt));\n\
         \x20\x20\x20\x20qg = (2.0 / 3.0) * cox * area * (vgte * vgte + vgte * vgdt + vgdt * vgdt) / (vgte + vgdt + 1.0e-9);\n\
         \x20\x20\x20\x20fd = vgdt * vgdt / (vgte * vgte + vgdt * vgdt + 1.0e-18);\n\
         \x20\x20\x20\x20I(g, si) <+ ddt(TYPE * qg * (1.0 - fd));\n\
         \x20\x20\x20\x20I(g, di) <+ ddt(TYPE * qg * fd);\n\
         \x20\x20\x20\x20// Small-signal operating point: gm/gds are the I-V slopes; the gate-charge\n\
         \x20\x20\x20\x20// derivatives are the real terminal capacitances (Cgs = Cgg - Cgd, so no ddx\n\
         \x20\x20\x20\x20// of the reference source node). These prove the AC model and read like psitft.\n\
         \x20\x20\x20\x20gm = ddx(TYPE * ids, V(g));\n\
         \x20\x20\x20\x20gds = ddx(TYPE * ids, V(d));\n\
         \x20\x20\x20\x20cgg = ddx(TYPE * qg, V(g));\n\
         \x20\x20\x20\x20cgd = -ddx(TYPE * qg, V(d));\n\
         \x20\x20\x20\x20cgs = cgg - cgd;\n\
         \x20\x20end\n\
         endmodule\n",
        ty = ty,
        vto = vto,
        vfb = vfb,
        mu0 = p.mu0,
        mu1 = p.mu1,
        mmu = p.mmu,
        mus = p.mus,
        asat = p.asat,
        lambda = p.lambda,
        delta = p.delta,
        eta = p.eta,
        vkink = p.vkink,
        lkink = p.lkink,
        mk = p.mk,
        i00 = p.i00,
        eb = p.eb,
        eps = p.eps,
        epsi = p.epsi,
        tox = p.tox,
        rs = p.rs,
        rd = p.rd,
        at = p.at,
        bt = p.bt,
        vsi = p.vsi,
        vst = p.vst,
        dvto = p.dvto,
        dmu1 = p.dmu1,
        dasat = p.dasat,
        lasat = p.lasat,
        w = geom.w_um,
        l = geom.l_um,
        tnom_k = temp_k,
    );
    va
}
