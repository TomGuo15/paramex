// Level 62-derived poly-Si forward model (ADRs 0012 and 0019). Two currents — an
// above-threshold square-law `Ia` and an exponential subthreshold `Isub` — use an
// analog-stabilized generalized-harmonic blend, while field-effect mobility rises then saturates (the
// poly-Si signature), and the impact-ionization KINK multiplies the current in
// saturation. Equations transcribed clean-room from the public HSPICE Level 62 manual
// and the Jacunski/Iniguez papers (cross-checked across three independent sources).

use super::forward::level62_current;
use super::{
    extract_level62, level62_output, level62_transfer, level62_verilog_a_card, refine_level62_dibl,
    refine_level62_output, Level62Params,
};
use crate::modelfit::{GeometryParams, OutputCurve, Polarity};

/// Independent transcription of the physical constants (so the expected values below
/// are not a re-use of the production constants).
const E0: f64 = 8.854_187_8e-12;
const Q: f64 = 1.602_176_634e-19;
const KB: f64 = 1.380_649e-23;
const T_NOM_K: f64 = 298.15;

fn unit_geom() -> GeometryParams {
    GeometryParams {
        w_um: 100.0,
        l_um: 100.0,
    } // W/L = 1
}

fn transfer_calibrated_seed(
    mut seed: Level62Params,
    truth: Level62Params,
    transfer_vds: f64,
) -> Level62Params {
    let seed_gain = seed.asat * (1.0 + seed.lambda * transfer_vds);
    let truth_gain = truth.asat * (1.0 + truth.lambda * transfer_vds);
    let scale = truth_gain / seed_gain;
    seed.mu0 *= scale;
    seed.mu1 *= scale;
    let truth_anchor = level62_current(&truth, unit_geom(), T_NOM_K, 4.8, transfer_vds);
    let seed_anchor = level62_current(&seed, unit_geom(), T_NOM_K, 4.8, transfer_vds);
    let scale = truth_anchor / seed_anchor;
    seed.mu0 *= scale;
    seed.mu1 *= scale;
    seed
}

mod dibl_fit;
mod export;
mod forward;
mod output_fit;
mod transfer_fit;
