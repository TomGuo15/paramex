//! Model-card export. Projects the extracted parameters into a self-contained,
//! compilable Verilog-A module — the Cadence-Spectre / ADS / ngspice-usable form
//! of ParamEx's *custom* compact models (there is no Spectre built-in for AOSTFT /
//! this Level 62, so a SPICE `.model` card can't carry them). The
//! module is `ahdl_include`d and instantiated directly; every parameter is an
//! instance-overridable `parameter real`, so no wrapper subckt is needed to tune
//! or corner it. Output-curve params (`alpha_sat`, `lambda`, `m`) and the off-state
//! params (subthreshold swing `SS`, leakage floor `IOFF`) are emitted from
//! extraction when available, otherwise at provisional defaults.

use super::extract::AboveThresholdFit;
use super::forward::{CHARGE_SMOOTH_V, VGTE_SMOOTH_V};
use super::types::{BiasParams, GeometryParams, OutputParams, Polarity, SubthresholdParams};

/// The finite-drain conductance coefficient: the UMEM H-function fits `K` as a
/// *current* coefficient at the transfer drain bias `v_ds`, so the conductance
/// gain is `K / v_ds` (guarded against a non-positive bias).
fn conductance_gain(k: f64, v_ds: f64) -> f64 {
    k / v_ds.max(f64::MIN_POSITIVE)
}

/// The output-curve params to emit: extracted values, or provisional defaults.
fn output_values(output: Option<&OutputParams>) -> (f64, f64, f64) {
    let o = output.copied().unwrap_or_else(OutputParams::card_defaults);
    (o.alpha_sat, o.lambda, o.m)
}

/// The off-state params to emit: extracted `(SS, IOFF)`, with any non-physical
/// extracted value (≤ 0 or non-finite) replaced per-field by the provisional
/// default. A noisy or wrong-polarity off-region can make `extract_subthreshold`
/// return a negative/zero SS; emitting it raw would write `SS = -3.2` into the
/// card and blow up its `exp(2.3*(Vgs-VTO)/SS)` in Cadence. A garbage SS is
/// effectively "no clean subthreshold region found", so we fall back to the same
/// default the `sub = None` path already uses — keeping every exported card a
/// valid, simulable Verilog-A.
fn subthreshold_values(sub: Option<&SubthresholdParams>) -> (f64, f64) {
    let s = sub
        .copied()
        .unwrap_or_else(SubthresholdParams::card_defaults);
    let d = SubthresholdParams::card_defaults();
    let ss = if s.ss_v_dec.is_finite() && s.ss_v_dec > 0.0 {
        s.ss_v_dec
    } else {
        d.ss_v_dec
    };
    let ioff = if s.ioff.is_finite() && s.ioff > 0.0 {
        s.ioff
    } else {
        d.ioff
    };
    (ss, ioff)
}

/// A self-contained, compilable Verilog-A module of the AOSTFT model with this
/// device's extracted parameters as defaults — the artifact to `ahdl_include`
/// and simulate in Cadence Spectre / ADS. It has a REAL off state following the
/// published UMEM/AOSTFT model (Iñiguez et al., IEEE JEDS 2021): the
/// above-threshold current `iab` is `tanh`-blended with an exponential
/// subthreshold branch `I_DSB = ids0*exp(2.3*(Vgs-VTO)/SS)` of slope = the
/// subthreshold swing `SS`. `ids0` is anchored for continuity at the crossover
/// `VTO+DV`, so the curve sews smoothly from the exponential subthreshold into
/// the power-law on-region — never a hard zero. An explicit `IOFF` leakage floor
/// is added on top (the paper notes gate leakage was not considered). The blend
/// is written in the overflow-safe sigmoid/softplus form (the literal `(1±tanh)/2`
/// weights would give `inf*0` for the subthreshold exp at high Vgs).
///
/// Above threshold: a channel CONDUCTANCE
/// `gch = (W/L)*KP*Vgte^(1+GAMMA) / (1 + RS*(W/L)*KP*Vgte^(1+GAMMA))` times the
/// effective drain `Vdse = Vds/(1+|Vds/Vdsat|^MSAT)^(1/MSAT)` and CLM:
/// `iab = gch*Vdse*(1+LAMBDA*Vds)`, `Vdsat = ALPHASAT*Vgte`. Because `Vdse` is NOT
/// normalized by `Vdsat`, in saturation `Vdse -> Vdsat = ALPHASAT*Vgte`, so
/// `iab ~ Vgte^(2+GAMMA)` — the strict published form. `KP` is the *per-square
/// conductance* gain (`K/VDS * L/W`): the H-function fits `K` as a current
/// coefficient at the transfer bias `VDS`, so `K/VDS` is the conductance and the
/// card reproduces the fit at `VDS` while extrapolating correctly in Vds/size.
/// `Vgte = (Vgt + sqrt(Vgt^2 + dlt^2))/2` is a smooth effective overdrive (C∞ gm).
/// The off-state adapts the published crossover with derivative-matched `DV`,
/// `QB = 2/SS`, smooth overdrive, and an explicit IOFF floor.
pub(super) fn verilog_a_card(
    device_name: &str,
    fit: &AboveThresholdFit,
    output: Option<&OutputParams>,
    sub: Option<&SubthresholdParams>,
    geom: GeometryParams,
    bias: BiasParams,
    polarity: Polarity,
) -> String {
    let model = sanitize_model_name(device_name);
    let (alpha_sat, lambda, msat) = output_values(output);
    let (ss, ioff) = subthreshold_values(sub);
    format!(
        "// ParamEx Model Fit \u{2014} AOSTFT compact model (Verilog-A) for device: {device_name}\n\
         // Reimplemented clean-room from the published UMEM/AOSTFT TFT model\n\
         // (In\u{0303}iguez et al., IEEE J. Electron Devices Soc. 2021; CC BY 4.0).\n\
         // Finite-drain channel current (saturation ~ (Vgs-VTO)^(2+GAMMA))\n\
         // plus a derivative-matched smooth above-/subthreshold crossover.\n\
         // KP is the per-square CONDUCTANCE gain, anchored at the transfer bias\n\
         // VDS={vds:.4e} V; current scales with W/L (W, L in um).\n\
         // Spectre:  ahdl_include \"{model}.va\"\n\
         //           M0 (vd vg vs) aostft\n\
         `include \"constants.vams\"\n\
         `include \"disciplines.vams\"\n\
         \n\
         module aostft(d, g, s);\n\
         \x20\x20inout d, g, s;\n\
         \x20\x20electrical d, g, s;\n\
         \x20\x20parameter real TYPE = {ty:.4e};\x20\x20// +1 n-channel, -1 p-channel\n\
         \x20\x20parameter real VTO = {vto:.4e};\n\
         \x20\x20parameter real GAMMA = {gamma:.4e};\n\
         \x20\x20parameter real KP = {kp:.4e};\n\
         \x20\x20parameter real W = {w:.4e};\x20\x20// bare um number or SI value, e.g. 1500 or 1500u\n\
         \x20\x20parameter real L = {l:.4e};\x20\x20// bare um number or SI value, e.g. 50 or 50u\n\
         \x20\x20parameter real RS = {rs:.4e};\n\
         \x20\x20parameter real ALPHASAT = {alpha_sat:.4e};\n\
         \x20\x20parameter real LAMBDA = {lambda:.4e};\n\
         \x20\x20parameter real MSAT = {msat:.4e};\n\
         \x20\x20parameter real SS = {ss:.4e};\n\
         \x20\x20parameter real IOFF = {ioff:.4e};\n\
         \x20\x20parameter real COX = {cox:.4e};\x20\x20// gate cap per area (F/m^2); 0 = DC-only\n\
         \x20\x20real vgt, vd, w_m, l_m, wl, dlt, vgte, vgtec, dv, qb, gch, gchc;\n\
         \x20\x20real vdsat, vdse, iab, vdsatc, vdsec, iabc, ids0, w, ids_above, sp2w, below, ids;\x20\x20// 'above' is a reserved Verilog-AMS operator -> ids_above\n\
         \x20\x20real vgdt, area, qg, fd, vgtq, dlq;\n\
         \x20\x20// Operating-point outputs (Spectre Results Display / ngspice OSDI): the standard\n\
         \x20\x20// small-signal set, so the model reads like a real device (and shows it has AC).\n\
         \x20\x20(* desc=\"DC transconductance dId/dVgs\", units=\"S\" *) real gm;\n\
         \x20\x20(* desc=\"DC output conductance dId/dVds\", units=\"S\" *) real gds;\n\
         \x20\x20(* desc=\"gate capacitance Cgg=dQg/dVg\", units=\"F\" *) real cgg;\n\
         \x20\x20(* desc=\"gate-drain capacitance\", units=\"F\" *) real cgd;\n\
         \x20\x20(* desc=\"gate-source capacitance\", units=\"F\" *) real cgs;\n\
         \x20\x20analog begin\n\
         \x20\x20\x20\x20// TYPE folds p- and n-channel into one module: work in the on-direction\n\
         \x20\x20\x20\x20// frame (vgt>0 when on), then send the current back out with TYPE.\n\
         \x20\x20\x20\x20vgt = TYPE * (V(g, s) - VTO);\n\
         \x20\x20\x20\x20vd = TYPE * V(d, s);\n\
         \x20\x20\x20\x20// ponytail: accepts bare um or Cadence suffix values; add explicit unit param if sub-0.01um devices matter.\n\
         \x20\x20\x20\x20w_m = (W < 1.0e-2) ? W : W * 1.0e-6;\n\
         \x20\x20\x20\x20l_m = max((L < 1.0e-2) ? L : L * 1.0e-6, 1.0e-12);\n\
         \x20\x20\x20\x20wl = w_m / l_m;\n\
         \x20\x20\x20\x20dlt = {smooth:.4e};\x20\x20// overdrive smoothing (V)\n\
         \x20\x20\x20\x20// Match the strict saturation branch's log-slope to ln(10)/SS at the crossover.\n\
         \x20\x20\x20\x20dv = sqrt(max(((2.0 + GAMMA) * SS / ln(10.0)) * ((2.0 + GAMMA) * SS / ln(10.0)) - dlt * dlt, 0.0));\n\
         \x20\x20\x20\x20qb = 2.0 / SS;\x20\x20// blend slope (paper Q): 2*qb > 2.3/SS keeps it stable\n\
         \x20\x20\x20\x20// smooth effective overdrive (C-inf -> no threshold kink in gm/caps)\n\
         \x20\x20\x20\x20vgte = 0.5 * (vgt + sqrt(vgt * vgt + dlt * dlt));\n\
         \x20\x20\x20\x20// I_DSA: channel conductance * effective drain * CLM, with series R\n\
         \x20\x20\x20\x20gch = wl * KP * pow(vgte, 1.0 + GAMMA);\n\
         \x20\x20\x20\x20gch = gch / (1.0 + RS * gch);\n\
         \x20\x20\x20\x20vdsat = ALPHASAT * vgte;\n\
         \x20\x20\x20\x20vdse = vd / pow(1.0 + pow(abs(vd / max(vdsat, 1.0e-9)), MSAT), 1.0 / MSAT);\n\
         \x20\x20\x20\x20iab = gch * vdse * (1.0 + LAMBDA * vd);\n\
         \x20\x20\x20\x20// I_DSA at the crossover VTO+DV -> subthreshold continuity anchor ids0\n\
         \x20\x20\x20\x20vgtec = 0.5 * (dv + sqrt(dv * dv + dlt * dlt));\n\
         \x20\x20\x20\x20gchc = wl * KP * pow(vgtec, 1.0 + GAMMA);\n\
         \x20\x20\x20\x20gchc = gchc / (1.0 + RS * gchc);\n\
         \x20\x20\x20\x20vdsatc = ALPHASAT * vgtec;\n\
         \x20\x20\x20\x20vdsec = vd / pow(1.0 + pow(abs(vd / max(vdsatc, 1.0e-9)), MSAT), 1.0 / MSAT);\n\
         \x20\x20\x20\x20iabc = gchc * vdsec * (1.0 + LAMBDA * vd);\n\
         \x20\x20\x20\x20ids0 = iabc * exp(-ln(10.0) * dv / SS);\n\
         \x20\x20\x20\x20// Smooth crossover in overflow-safe sigmoid/softplus form, + IOFF leakage floor\n\
         \x20\x20\x20\x20w = (vgt - dv) * qb;\n\
         \x20\x20\x20\x20ids_above = iab / (1.0 + exp(-2.0 * w));\n\
         \x20\x20\x20\x20sp2w = max(2.0 * w, 0.0) + ln(1.0 + exp(-abs(2.0 * w)));\n\
         \x20\x20\x20\x20below = ids0 * exp(ln(10.0) * vgt / SS - sp2w);\n\
         \x20\x20\x20\x20ids = ids_above + below + IOFF;\n\
         \x20\x20\x20\x20I(d, s) <+ TYPE * ids;\n\
         \x20\x20\x20\x20// AC/transient: charge-based gate model (gate-channel charge Cox*W*L).\n\
         \x20\x20\x20\x20// Total gate charge has the verified Meyer limits (Cgg: Cox*W*L triode ->\n\
         \x20\x20\x20\x20// 2/3*Cox*W*L saturation); the source/drain split fd reproduces Cgd->0 in\n\
         \x20\x20\x20\x20// saturation, Cgs=Cgd in triode. COX=0 -> zero caps (DC-only).\n\
         \x20\x20\x20\x20area = w_m * l_m;\x20\x20// m^2 for COX(F/m^2)\n\
         \x20\x20\x20\x20dlq = {smooth_q:.4e};\x20\x20// gate-CHARGE turn-on, wider than dlt so Cgg co-locates with gm (AC-only)\n\
         \x20\x20\x20\x20vgtq = 0.5 * (vgt + sqrt(vgt * vgt + dlq * dlq));\n\
         \x20\x20\x20\x20vgdt = 0.5 * ((vgtq - vd) + sqrt((vgtq - vd) * (vgtq - vd) + dlt * dlt));\n\
         \x20\x20\x20\x20qg = (2.0 / 3.0) * COX * area * (vgtq * vgtq + vgtq * vgdt + vgdt * vgdt) / (vgtq + vgdt + 1.0e-9);\n\
         \x20\x20\x20\x20fd = vgdt * vgdt / (vgtq * vgtq + vgdt * vgdt + 1.0e-18);\n\
         \x20\x20\x20\x20I(g, s) <+ ddt(TYPE * qg * (1.0 - fd));\n\
         \x20\x20\x20\x20I(g, d) <+ ddt(TYPE * qg * fd);\n\
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
        device_name = device_name,
        vds = bias.v_ds,
        model = model,
        ty = polarity.sign(),
        vto = fit.vt,
        gamma = fit.gamma,
        kp = geom.per_square_kp(conductance_gain(fit.k, bias.v_ds)),
        w = geom.w_um,
        l = geom.l_um,
        rs = bias.r,
        cox = bias.cox,
        smooth = VGTE_SMOOTH_V,
        smooth_q = CHARGE_SMOOTH_V,
        alpha_sat = alpha_sat,
        lambda = lambda,
        msat = msat,
        ss = ss,
        ioff = ioff,
    )
}

/// Collapse a device name to a SPICE identifier: non-alphanumerics become single
/// underscores, with leading/trailing underscores trimmed. Shared by every model's
/// `.model`/`.va` card export so cards name devices consistently.
pub(super) fn sanitize_model_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_underscore = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_underscore = false;
        } else if !prev_underscore {
            out.push('_');
            prev_underscore = true;
        }
    }
    let stem = out.trim_matches('_');
    if stem.is_empty() {
        "model".to_string()
    } else {
        stem.to_string()
    }
}

pub(super) fn model_card_filename(device_name: &str) -> String {
    format!("{}.va", sanitize_model_name(device_name))
}

#[cfg(test)]
mod tests {
    use super::{model_card_filename, sanitize_model_name};

    #[test]
    fn model_name_and_filename_share_one_total_sanitizer() {
        assert_eq!(sanitize_model_name(" lot A / TFT #7 "), "lot_A_TFT_7");
        assert_eq!(model_card_filename(" lot A / TFT #7 "), "lot_A_TFT_7.va");
        assert_eq!(sanitize_model_name("///"), "model");
        assert_eq!(model_card_filename("///"), "model.va");
    }
}

#[cfg(test)]
#[path = "tests/export.rs"]
mod integration_tests;

#[cfg(test)]
#[path = "tests/charge.rs"]
mod charge_tests;
