//! Compact-model fitting product seam.
//!
//! AOSTFT / UMEM is the default model; Level 62-derived (LTPS, stabilized) is its sibling.

mod ac;
mod device;
mod export;
mod extract;
mod forward;
mod level62;
mod optimize;
mod parse;
mod types;

fn transfer_preservation_log_r2(transfer: (&[f64], &[f64]), predict: impl Fn(f64) -> f64) -> f64 {
    let (transfer_vg, transfer_id) = transfer;
    let n = transfer_vg.len().min(transfer_id.len());
    if n < 5 {
        return 1.0;
    }
    let measured: Vec<_> = transfer_id[..n]
        .iter()
        .map(|id| id.max(1.0e-30).log10())
        .collect();
    let mean = measured.iter().sum::<f64>() / n as f64;
    let total = measured
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>();
    let residual = transfer_vg[..n]
        .iter()
        .zip(&measured)
        .map(|(&vg, &measured)| (predict(vg).max(1.0e-30).log10() - measured).powi(2))
        .sum::<f64>();
    1.0 - residual / total
}

pub use device::{
    AnalogFitQuality, DetachDiblError, DetachOutputError, DiblError, DiblReplacement,
    DiblReplacementError, EditError, FitDeviceError, FitModel, FittedDevice, FittedModelView,
    InputError, ModelCardArtifact, OutputAttachOutcome, OutputReplacement, OutputReplacementError,
    OutputSeries, RefitError,
};
pub use extract::AboveThresholdFit;
pub use level62::{Level62Fit, Level62Params};
pub use parse::{
    extract_accumulation_capacitance_file, parse_output_file, parse_second_transfer_file,
    SecondTransfer,
};
pub use types::{
    BiasParams, GeometryParams, ModelParams, OutputCurve, OutputParams, Polarity,
    SubthresholdParams,
};

/// Measurement-container extensions accepted by the Model Fit file parsers.
pub const SUPPORTED_EXTENSIONS: [&str; 5] = crate::shared::grid_ingest::MEASUREMENT_EXTENSIONS;
