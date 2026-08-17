//! Poly-Si **Level 62-derived, analog-stabilized** TFT compact model for LTPS /
//! poly-silicon devices.
//!
//! Clean-room from the public *HSPICE MOSFET Models Manual* (X-2005.09) "Level 62 RPI
//! Poli-Si TFT Model" (pp. 272–275), cross-checked against the canonical papers
//! (Jacunski/Shur, IEEE T-ED 46(6) 1999; Iniguez et al., Solid-State Electronics 43(9)
//! 1999) and the Iniguez MOS-AK 2015 slides. Unlike the a-Si Level 61 sibling — which
//! harmonic-means two *charge* densities — Level 62 computes two *currents* (an
//! above-threshold square-law `Ia` and an exponential subthreshold `Isub`). The canonical
//! model harmonic-means those currents; ParamEx uses an order-1/2 generalized
//! harmonic interpolation, adds leakage, and multiplies by the
//! impact-ionization **kink** factor. Its mobility *rises then saturates* with gate
//! bias (grain-boundary-barrier lowering), the poly-Si signature a-Si lacks:
//!
//! ```text
//! Ids    = [ Ichan + Ileak ]·(1 + Ikink)
//! Ichan  = Imin/[1 + sqrt(Imin/Imax)]², Imin/max = min/max(Ia, Isub)  # stabilized blend
//! 1/µFET = 1/MU0 + 1/( MU1·(2·VGTE/Vsth)^MMU )                   # rises with Vgs → saturates to MU0
//! Ia     = µFET·Cox·(W/L)·(VGTE·Vds − Vds²/(2·αsat)),  Vds ≤ Vdsat   (triode)
//!        = µFET·Cox·(W/L)·αsat·VGTE²/2·(1+LAMBDA·Vds),  Vds > Vdsat   (saturation)
//! Vdsat  = αsat·VGTE
//! Isub   = MUS·Cox·(W/L)·Vsth²·exp(VGT/Vsth)·[1 − exp(−Vds/Vsth)]  # diffusion-like exponential
//! Vsth   = ETA·Vth,   Vth = kT/q,   Cox = EPSI·ε0/TOX,   VGT = Vgs − VTO
//! VGTE   = (Vmin/2)·[1 + VGT/Vmin + sqrt(DELTA² + (VGT/Vmin − 1)²)],  Vmin = 2·Vsth
//! Ikink  = (1/VKINK)·(LKINK/Leff)^MK·(Vds − Vdsk)·exp(−VKINK/(Vds − Vdsk))   # impact ionization
//! Vdsk   = Vds/[1 + (Vds/Vdsat)³]^(1/3) − Vth                     # effective drain for the kink
//! Ileak  = I00·W·exp(−EB/Vth)·[1 − exp(−Vds/Vth)]                 # reverse-diode off-floor (v1)
//! ```
//!
//! **DIBL + temperature structure:** the model carries
//! the manual's effective threshold and temperature scaling —
//!
//! ```text
//! VTeff = VTX − (AT·Vds² + BT)/( Leff·(1 + exp((Vgs − VST − VTX)/VSI)) )
//! VTX   = VTO − DVTO·(TEMP − TNOM)
//! µ1    = MU1 + DMU1·(TEMP − TNOM)
//! αsat  = ASAT − LASAT/Leff − DASAT·(TEMP − TNOM)
//! ```
//!
//! ParamEx defaults `AT = BT = 0` and zero temperature coefficients (a documented
//! deviation from the manual's `AT`/`BT` defaults), so extraction — which cannot
//! identify them from a single-`Vds`, single-temperature transfer — is unchanged; the
//! exported `.va` exposes them as instance-overridable knobs and follows the
//! simulator's `$temperature`. Still deferred: Version-2 velocity
//! saturation (equations unverified) and the `XTFE` thermionic-field-emission leakage
//! blend — the leakage here is the clean reverse-diode floor only. The **kink** lives
//! only in saturation (high `Vds`), so it is invisible on — and not extractable
//! from — a low-`Vds` transfer; its parameters are carried at the documented defaults
//! until an output family refines them.
//!
//! Inputs are the device's terminal voltages in the n-channel-on frame (`Vgs` rises
//! into conduction); polarity is folded by the caller, as for AOSTFT / Level 61.

mod export;
mod fit;
mod forward;
mod params;

pub(super) use export::level62_verilog_a_card;
pub use fit::Level62Fit;
pub(super) use fit::{extract_level62, refine_level62_dibl, refine_level62_output};
pub(super) use forward::{level62_output, level62_transfer};
pub use params::Level62Params;

#[cfg(test)]
mod tests;
