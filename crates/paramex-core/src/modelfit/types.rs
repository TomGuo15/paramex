//! Shared Model Fit domain types plus the AOSTFT / UMEM parameter structs.

/// Channel polarity of a fitted device. The UMEM H-function extraction is written
/// for an n-channel device (drain current rises as `Vgs` increases above `VT`);
/// a p-channel device (current rises as `Vgs` goes *below* `VT`) is handled by
/// flipping the gate axis (`Vgs -> -Vgs`) before extraction, then flipping the
/// recovered `VT` back. The exported card carries a `TYPE` sign so one module
/// serves both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// n-channel: on for `Vgs > VT`.
    NChannel,
    /// p-channel: on for `Vgs < VT`.
    PChannel,
}

impl Polarity {
    /// The sign folded into the model: `+1` for n-channel, `-1` for p-channel.
    pub fn sign(self) -> f64 {
        match self {
            Polarity::NChannel => 1.0,
            Polarity::PChannel => -1.0,
        }
    }

    /// Map a gate voltage between the device frame and the n-channel extraction
    /// frame (negate for p-channel). An involution, so it also maps `VT` back.
    pub fn map_vg(self, vg: f64) -> f64 {
        self.sign() * vg
    }
}

/// AOSTFT above-threshold model parameters.
///
/// `k` is the directly-fitted gain in `Id = k * (Vg - VT)^(1 + gamma)`; the
/// physical band mobility and the secondary (output-curve) parameters are added
/// in later phases as the extraction grows.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelParams {
    /// Threshold voltage `VT` (V).
    pub vt: f64,
    /// Mobility exponent `gamma` (dimensionless); `mu_FE ∝ (Vg - VT)^gamma`.
    pub gamma: f64,
    /// Gain prefactor `K` in `Id = K * (Vg - VT)^(1 + gamma)`.
    pub k: f64,
}

/// AOSTFT output-curve (saturation) parameters, extracted from Id-Vd curves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputParams {
    /// Saturation-voltage ratio: `V_dsat = alpha_sat * (Vg - VT)`.
    pub alpha_sat: f64,
    /// Channel-length-modulation coefficient (V^-1).
    pub lambda: f64,
    /// Saturation-knee sharpness.
    pub m: f64,
}

impl OutputParams {
    /// The provisional output-curve defaults a card falls back to when no Id-Vd
    /// curves were measured. The single source of truth shared by the
    /// model-card export and the *predicted* output plot, so the plot shows exactly
    /// the saturation shape the exported card carries.
    pub const fn card_defaults() -> Self {
        OutputParams {
            alpha_sat: 0.6,
            lambda: 1.0e-3,
            m: 2.5,
        }
    }
}

/// AOSTFT off-state / subthreshold parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubthresholdParams {
    /// Subthreshold swing `S` (V/decade) — the off-region slope.
    pub ss_v_dec: f64,
    /// Off-state leakage floor `Ioff` (A).
    pub ioff: f64,
}

impl SubthresholdParams {
    /// The provisional off-state defaults a card falls back to when no clean
    /// subthreshold region was found. Single source of truth shared by
    /// the model-card export and the predicted/overlay output current, so the plot
    /// reproduces the exported card's off-state blend.
    pub const fn card_defaults() -> Self {
        SubthresholdParams {
            ss_v_dec: 0.3,
            ioff: 1.0e-12,
        }
    }
}

/// Device channel geometry (µm). The AOSTFT model current is `∝ W/L`, so the
/// exported card carries `W`, `L` as real parameters and a *per-square* gain
/// (`KP = K·L/W`) — letting the simulator instantiate the device at any size
/// while reproducing the fit at this device's own `W/L`. Units cancel in `W/L`,
/// so any consistent length unit works; we label it µm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryParams {
    /// Channel width `W` (µm).
    pub w_um: f64,
    /// Channel length `L` (µm).
    pub l_um: f64,
}

impl Default for GeometryParams {
    /// Same default geometry as the Transfer page (`W/L = 30`) so the two
    /// workspaces start from one device-size convention.
    fn default() -> Self {
        GeometryParams {
            w_um: 1500.0,
            l_um: 50.0,
        }
    }
}

impl GeometryParams {
    /// Per-square gain `KP = g·L/W` for a per-device coefficient `g`, so
    /// `(W/L)·KP = g`. Used to fold geometry out of the conductance gain.
    pub fn per_square_kp(self, g: f64) -> f64 {
        g * self.l_um / self.w_um.max(f64::MIN_POSITIVE)
    }
}

/// Per-device bias + process inputs the strict (Eq. 25) card and the AC charge
/// model need beyond the extracted parameters.
///
/// - `v_ds` anchors the conductance: the UMEM H-function fits `K` as a *current*
///   coefficient at the transfer's drain bias, so the strict model's conductance
///   coefficient is `K / v_ds` (the transfer must be a linear-region sweep).
/// - `cox` (gate capacitance per area, F/m²) drives the AC gate-charge model;
///   `0` disables it (DC-only fallback).
/// - `r` is series resistance (Ω); `0` = ideal contacts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiasParams {
    /// Transfer measurement drain bias `V_DS` (V) — the linear-region sweep bias.
    pub v_ds: f64,
    /// Gate capacitance per unit area `Cox` (F/m²); `0` disables the AC charge model.
    pub cox: f64,
    /// Series resistance `R` (Ω); `0` = ideal.
    pub r: f64,
}

impl Default for BiasParams {
    /// A linear-region transfer bias, Transfer's default Cox (`10 nF/cm²`), and
    /// ideal contacts. `v_ds` is Model-Fit-specific; Transfer has no matching
    /// drain-bias setting.
    fn default() -> Self {
        BiasParams {
            v_ds: 0.1,
            cox: 1.0e-4,
            r: 0.0,
        }
    }
}

/// One measured output sub-sweep: `Id` vs `Vd` at a fixed gate voltage.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputCurve {
    /// The fixed gate voltage of this sub-sweep.
    pub vg: f64,
    /// Drain-voltage samples (ascending).
    pub vds: Vec<f64>,
    /// Drain current at each `vds`.
    pub id: Vec<f64>,
}
