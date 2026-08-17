use crate::modelfit::types::GeometryParams;

/// Poly-Si **Level 62-derived** DC parameter set (AIM-SPICE MOS16 names). SI units
/// throughout (`TOX` in m, mobilities in m²·V⁻¹·s⁻¹, resistances in Ω); geometry is
/// supplied separately in µm via [`GeometryParams`]. Carries the verified DIBL and
/// temperature structure with no-op defaults; Version-2 and the detailed
/// field-emission-leakage parameters are omitted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level62Params {
    /// `VTO` — zero-bias threshold voltage (V).
    pub vto: f64,
    /// `VFB` — flat-band voltage (V); used by the (deferred) leakage field terms.
    pub vfb: f64,
    /// `MU0` — high-field band mobility, the saturating ceiling of µFET (m²·V⁻¹·s⁻¹).
    pub mu0: f64,
    /// `MU1` — low-field mobility coefficient in the rise-then-saturate µFET (m²·V⁻¹·s⁻¹).
    pub mu1: f64,
    /// `MMU` — low-field mobility power-law exponent (–).
    pub mmu: f64,
    /// `MUS` — subthreshold (diffusion) mobility prefactor (m²·V⁻¹·s⁻¹).
    pub mus: f64,
    /// `ASAT` — saturation/body-effect parameter σsat: `Vdsat = σsat·VGTE` (–).
    pub asat: f64,
    /// `LAMBDA` — saturation channel-length-modulation parameter (V⁻¹; Version-2, default 0).
    pub lambda: f64,
    /// `DELTA` — transition-width smoothing for `VGTE` (–).
    pub delta: f64,
    /// `ETA` — subthreshold ideality factor: `Vsth = ETA·Vth`; sets `SS = ln10·ETA·kT/q` (–).
    pub eta: f64,
    /// `VKINK` — kink-effect (impact-ionization) characteristic voltage (V).
    pub vkink: f64,
    /// `LKINK` — kink-effect length constant (m).
    pub lkink: f64,
    /// `MK` — kink-effect length-scaling exponent (–).
    pub mk: f64,
    /// `I00` — reverse-diode leakage prefactor (A·m⁻¹).
    pub i00: f64,
    /// `EB` — leakage-diode barrier height (eV, numerically = V in `exp(−EB/Vth)`).
    pub eb: f64,
    /// `EPS` — relative dielectric constant of the poly-Si film (–; used by the deferred leakage).
    pub eps: f64,
    /// `EPSI` — relative dielectric constant of the gate insulator (–).
    pub epsi: f64,
    /// `TOX` — gate-insulator thickness (m).
    pub tox: f64,
    /// `RS` — source series resistance (Ω; deferred, default 0).
    pub rs: f64,
    /// `RD` — drain series resistance (Ω; deferred, default 0).
    pub rd: f64,
    /// `AT` — DIBL parameter 1, the `Vds²` threshold-lowering strength (m·V⁻¹).
    /// ParamEx default 0 (DIBL off; manual default 3e-8).
    pub at: f64,
    /// `BT` — DIBL parameter 2, the bias-independent threshold offset (m·V).
    /// ParamEx default 0 (DIBL off; manual default 1.9e-6).
    pub bt: f64,
    /// `VSI` — DIBL gate-dependence width in `exp((Vgs − VST − VTX)/VSI)` (V).
    pub vsi: f64,
    /// `VST` — DIBL gate-dependence offset in `exp((Vgs − VST − VTX)/VSI)` (V).
    pub vst: f64,
    /// `DVTO` — temperature coefficient of `VTO`: `VTX = VTO − DVTO·ΔT` (V·K⁻¹).
    pub dvto: f64,
    /// `DMU1` — temperature coefficient of `MU1`: `µ1 = MU1 + DMU1·ΔT` (m²·V⁻¹·s⁻¹·K⁻¹).
    pub dmu1: f64,
    /// `DASAT` — temperature coefficient of `ASAT`: `αsat = ASAT − LASAT/Leff − DASAT·ΔT` (K⁻¹).
    pub dasat: f64,
    /// `LASAT` — channel-length dependence of `ASAT` (m).
    pub lasat: f64,
    /// `TNOM` — parameter measurement temperature (K); `ΔT = TEMP − TNOM`.
    pub tnom_k: f64,
}

impl Default for Level62Params {
    /// Canonical RPI poly-Si Level 62 defaults (AIM-SPICE MOS16), converted to SI.
    /// (Mobilities entered in the manual as cm²/Vs are ×1e-4 here.)
    fn default() -> Self {
        Level62Params {
            vto: 0.0,
            vfb: -0.1,
            mu0: 100.0e-4,  // 100 cm²/Vs
            mu1: 0.0022e-4, // 0.0022 cm²/Vs
            mmu: 3.0,
            mus: 1.0e-4, // 1 cm²/Vs
            asat: 1.0,
            lambda: 0.0,
            delta: 4.0,
            eta: 7.0,
            vkink: 9.1,
            lkink: 19.0e-6,
            mk: 1.3,
            i00: 150.0,
            eb: 0.68,
            eps: 11.7,
            epsi: 3.9,
            tox: 1.0e-7,
            rs: 0.0,
            rd: 0.0,
            // DIBL off by default (deliberate deviation from the manual's AT=3e-8 /
            // BT=1.9e-6 because they are not identifiable from one transfer). VSI/VST
            // carry the manual defaults; they are inert while AT = BT = 0.
            at: 0.0,
            bt: 0.0,
            vsi: 2.0,
            vst: 2.0,
            dvto: 0.0,
            dmu1: 0.0,
            dasat: 0.0,
            lasat: 0.0,
            tnom_k: 298.15, // TNOM = 25 °C, the manual's Synopsys default
        }
    }
}

impl Level62Params {
    /// Representative LTPS starting values (higher band mobility than the a-Si:H
    /// template, low-temperature poly-Si gate stack). A seed for fitting — the
    /// extractor refines the transfer-identifiable parameters from the measured I-V.
    pub fn ltps() -> Self {
        Level62Params {
            mu0: 120.0e-4,
            mu1: 0.05e-4,
            mmu: 2.5,
            mus: 5.0e-4,
            eta: 4.0,
            tox: 25.0e-9,
            ..Level62Params::default()
        }
    }

    pub(in crate::modelfit) fn is_valid_for(&self, geometry: GeometryParams) -> bool {
        let values = [
            self.vto,
            self.vfb,
            self.mu0,
            self.mu1,
            self.mmu,
            self.mus,
            self.asat,
            self.lambda,
            self.delta,
            self.eta,
            self.vkink,
            self.lkink,
            self.mk,
            self.i00,
            self.eb,
            self.eps,
            self.epsi,
            self.tox,
            self.rs,
            self.rd,
            self.at,
            self.bt,
            self.vsi,
            self.vst,
            self.dvto,
            self.dmu1,
            self.dasat,
            self.lasat,
            self.tnom_k,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return false;
        }
        let l = geometry.l_um * 1.0e-6;
        let asat = self.asat - self.lasat / l;
        let dielectric_ratio = self.epsi / self.tox;
        self.mu0 > 0.0
            && self.mu1 > 0.0
            && self.mmu > 0.0
            && self.mus > 0.0
            && self.asat > 0.0
            && asat.is_finite()
            && asat > 0.0
            && self.lambda >= 0.0
            && self.delta > 0.0
            && self.eta > 0.0
            && self.vkink > 0.0
            && self.lkink >= 0.0
            && self.mk > 0.0
            && self.i00 >= 0.0
            && self.eb >= 0.0
            && self.eps > 0.0
            && self.epsi > 0.0
            && self.tox > 0.0
            && dielectric_ratio.is_finite()
            && dielectric_ratio > 0.0
            && self.rs >= 0.0
            && self.rd >= 0.0
            && self.at >= 0.0
            && self.bt >= 0.0
            && self.vsi > 0.0
            && self.lasat >= 0.0
            && self.tnom_k > 0.0
    }
}
