//! The in-process fitted-device lifecycle shared by Model Fit callers.

use std::cell::{Cell, OnceCell};
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

use super::ac;
use super::extract::{
    extract_above_threshold, extract_output, extract_subthreshold, prepare_transfer,
    AboveThresholdFit,
};
use super::forward::{output_card_current, unified_transfer};
use super::level62::{
    extract_level62, level62_transfer, refine_level62_dibl, refine_level62_output, Level62Fit,
    Level62Params,
};
use super::parse::SecondTransfer;
use super::types::{
    BiasParams, GeometryParams, ModelParams, OutputCurve, OutputParams, Polarity,
    SubthresholdParams,
};
use crate::shared::numpy_compat::gradient;

mod aostft_output;
mod view;

/// Measurement temperature used by the Level 62 / LTPS lifecycle and export.
const LEVEL62_TNOM_K: f64 = 298.15;
const GM_ID_CEILING_V_INV: f64 = 38.7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FitModel {
    Aostft,
    Level62,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitDeviceError {
    InvalidTransferSamples,
    NoExtractableAboveThreshold,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    InvalidGeometry,
    InvalidBias,
    InvalidAccumulationCapacitance,
    InvalidAostftCardMapping,
    RetainedDiblNotApplied,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    NonFiniteAostftFit,
    InvalidAostftFit,
    InvalidSubthreshold,
    InvalidOutput,
    NoLevel62Fit,
    InvalidLevel62Params,
    InvalidAostftCardMapping,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefitError {
    ExtractionFailed,
    RetainedDiblNotApplied,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachOutputError {
    NoOutput,
    RetainedDiblNotApplied,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachDiblError {
    NoSecondTransfer,
    CannotRestoreLevel62Fit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputAttachOutcome {
    Fitted,
    NoFit,
}
/// The scientific result of replacing an attached output family. `displaced`
/// lets the workspace move its matching source metadata to a pending row without
/// losing the previous measured curves.
#[must_use = "displaced output curves must be handled"]
#[derive(Debug, Clone, PartialEq)]
pub struct OutputReplacement {
    pub outcome: OutputAttachOutcome,
    pub displaced: Vec<OutputCurve>,
}
/// An output family rejected before mutation because it would leave a retained
/// second transfer unapplied. The exact proposed curves remain caller-owned.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputReplacementError {
    RetainedDiblNotApplied { rejected: Vec<OutputCurve> },
}
/// The scientific result of replacing the retained second transfer.
/// `displaced` returns the exact prior measurement to the workflow that owns
/// its source provenance.
#[must_use = "displaced second-transfer data must be handled"]
#[derive(Debug, Clone, PartialEq)]
pub struct DiblReplacement {
    pub at: f64,
    pub displaced: Option<SecondTransfer>,
}
/// A proposed second transfer rejected before mutation. The exact measurement
/// remains caller-owned together with the scientific rejection reason.
#[derive(Debug, Clone, PartialEq)]
pub struct DiblReplacementError {
    pub reason: DiblError,
    pub rejected: SecondTransfer,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiblError {
    InvalidSecondTransfer,
    Level62Manual,
    Level62Unavailable,
    BiasTooClose,
    NoImprovement,
}

/// One gate sub-sweep represented in the normalized output-display frame.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSeries {
    pub vg: f64,
    pub measured: Vec<[f64; 2]>,
    pub modelled: Vec<[f64; 2]>,
}

/// A rendered model card and the canonical filename that its own include
/// guidance references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelCardArtifact {
    pub text: String,
    pub suggested_file_name: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnalogFitQuality {
    pub gm_p90: Option<f64>,
    pub gds_p90: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
struct AostftCardProjection {
    fit: AboveThresholdFit,
    output: OutputParams,
}

#[derive(Clone, Debug)]
struct AostftState {
    h_fit: AboveThresholdFit,
    subthreshold: Option<SubthresholdParams>,
    output: Option<OutputParams>,
    manual: bool,
    card_fit_cache: OnceCell<Option<AboveThresholdFit>>,
    analog_quality_cache: Cell<Option<AnalogFitQuality>>,
}

impl AostftState {
    fn new(h_fit: AboveThresholdFit, subthreshold: Option<SubthresholdParams>) -> Self {
        Self {
            h_fit,
            subthreshold,
            output: None,
            manual: false,
            card_fit_cache: OnceCell::new(),
            analog_quality_cache: Cell::new(None),
        }
    }

    fn card_fit(&self, bias: BiasParams) -> Option<&AboveThresholdFit> {
        let output = self.output.unwrap_or_else(OutputParams::card_defaults);
        self.card_fit_cache
            .get_or_init(|| map_h_fit_to_card(self.h_fit, output, bias))
            .as_ref()
    }

    fn card_projection(&self, bias: BiasParams) -> Option<AostftCardProjection> {
        Some(AostftCardProjection {
            fit: *self.card_fit(bias)?,
            output: self.output.unwrap_or_else(OutputParams::card_defaults),
        })
    }

    fn invalidate_card_fit(&mut self) {
        self.card_fit_cache.take();
    }
}

impl PartialEq for AostftState {
    fn eq(&self, other: &Self) -> bool {
        self.h_fit == other.h_fit
            && self.subthreshold == other.subthreshold
            && self.output == other.output
            && self.manual == other.manual
    }
}

#[derive(Clone, Debug)]
struct Level62State {
    fit: Option<Level62Fit>,
    manual: bool,
    analog_quality_cache: Cell<Option<AnalogFitQuality>>,
}

impl Level62State {
    fn new(fit: Option<Level62Fit>) -> Self {
        Self {
            fit,
            manual: false,
            analog_quality_cache: Cell::new(None),
        }
    }
}

impl PartialEq for Level62State {
    fn eq(&self, other: &Self) -> bool {
        self.fit == other.fit && self.manual == other.manual
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DiblTransfer {
    vg: Vec<f64>,
    id: Vec<f64>,
    v_ds: f64,
}

impl From<DiblTransfer> for SecondTransfer {
    fn from(second: DiblTransfer) -> Self {
        Self {
            vg: second.vg,
            id_abs: second.id,
            v_ds: second.v_ds,
        }
    }
}

struct Level62Rebuild {
    fit: Level62Fit,
    output_fitted: bool,
    dibl_fitted: bool,
}

enum RetainedDiblError {
    NotApplied,
}

/// A measured device and every scientific result derived from it. UI selection,
/// source paths, file pairing, and checkboxes deliberately do not live here.
#[derive(Clone, Debug)]
pub struct FittedDevice {
    name: String,
    vgs: Vec<f64>,
    id: Vec<f64>,
    aostft: AostftState,
    output_curves: Vec<OutputCurve>,
    geometry: GeometryParams,
    bias: BiasParams,
    polarity: Polarity,
    level62: Level62State,
    dibl_transfer: Option<DiblTransfer>,
}

impl PartialEq for FittedDevice {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.vgs == other.vgs
            && self.id == other.id
            && self.aostft == other.aostft
            && self.output_curves == other.output_curves
            && self.geometry == other.geometry
            && self.bias == other.bias
            && self.polarity == other.polarity
            && self.level62 == other.level62
            && self.dibl_transfer == other.dibl_transfer
    }
}

/// A model-specific, on-demand projection of one fitted device.
pub struct FittedModelView<'a> {
    device: &'a FittedDevice,
    kind: FitModel,
}

const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<FittedDevice>();
};

impl FittedDevice {
    pub fn fit(name: String, vgs: Vec<f64>, id: Vec<f64>) -> Result<Self, FitDeviceError> {
        if vgs.len() != id.len() || !vgs.iter().chain(&id).all(|sample| sample.is_finite()) {
            return Err(FitDeviceError::InvalidTransferSamples);
        }
        let prepared =
            prepare_transfer(&vgs, &id).ok_or(FitDeviceError::NoExtractableAboveThreshold)?;
        let mut fit = extract_above_threshold(prepared.vg(), prepared.id())
            .ok_or(FitDeviceError::NoExtractableAboveThreshold)?;
        let polarity = prepared.polarity();
        fit.vt = polarity.map_vg(fit.vt);
        let geometry = GeometryParams::default();
        let bias = BiasParams {
            v_ds: saturation_default_vds(&vgs, polarity),
            ..BiasParams::default()
        };
        let level62 = fit_level62(&vgs, &id, geometry, bias.v_ds);
        let mut device = Self {
            name,
            vgs,
            id,
            aostft: AostftState::new(fit, extract_subthreshold(prepared.vg(), prepared.id())),
            output_curves: Vec::new(),
            geometry,
            bias,
            polarity,
            level62: Level62State::new(level62),
            dibl_transfer: None,
        };
        device.recompute_model_r2(FitModel::Aostft);
        device.recompute_model_r2(FitModel::Level62);
        Ok(device)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn geometry(&self) -> GeometryParams {
        self.geometry
    }
    pub fn bias(&self) -> BiasParams {
        self.bias
    }
    pub fn polarity(&self) -> Polarity {
        self.polarity
    }
    pub fn aostft_fit(&self) -> &AboveThresholdFit {
        if self.aostft.output.is_some() {
            self.aostft
                .card_fit(self.bias)
                .expect("AOSTFT output mutations preserve a valid card projection")
        } else {
            &self.aostft.h_fit
        }
    }
    pub fn subthreshold(&self) -> Option<SubthresholdParams> {
        self.aostft.subthreshold
    }
    pub fn output(&self) -> Option<OutputParams> {
        self.aostft.output
    }
    pub fn level62(&self) -> Option<&Level62Fit> {
        self.level62.fit.as_ref()
    }
    pub fn has_output(&self) -> bool {
        self.aostft.output.is_some()
    }
    pub fn has_output_curves(&self) -> bool {
        !self.output_curves.is_empty()
    }
    pub fn has_second_transfer(&self) -> bool {
        self.dibl_transfer.is_some()
    }
    /// Whether the retained second transfer contributes to the current Level 62
    /// fit. Manual parameters deliberately suspend automatic DIBL refinement.
    pub fn is_second_transfer_applied(&self) -> bool {
        self.dibl_transfer.is_some() && !self.level62.manual
    }
    pub fn measured_points(&self) -> Vec<[f64; 2]> {
        self.vgs
            .iter()
            .zip(&self.id)
            .map(|(&v, &i)| [v, i])
            .collect()
    }
    pub fn vg_span(&self) -> (f64, f64) {
        let lo = self.vgs.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = self.vgs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if lo <= hi {
            (lo, hi)
        } else {
            (0.0, 0.0)
        }
    }
    pub fn model(&self, kind: FitModel) -> FittedModelView<'_> {
        FittedModelView { device: self, kind }
    }
    pub fn measured_gm_series(&self) -> Vec<[f64; 2]> {
        let Some(prepared) = prepare_transfer(&self.vgs, &self.id) else {
            return Vec::new();
        };
        gradient(prepared.id(), prepared.vg())
            .into_iter()
            .zip(prepared.vg().iter().copied())
            .map(|(g, v)| [self.polarity.map_vg(v), g])
            .collect()
    }
    pub fn measured_gm_id_sizing_series(&self) -> Vec<[f64; 2]> {
        let Some(prepared) = prepare_transfer(&self.vgs, &self.id) else {
            return Vec::new();
        };
        if prepared.vg().len() < 2 || !(self.geometry.w_um.is_finite() && self.geometry.w_um > 0.0)
        {
            return Vec::new();
        }
        let floor = prepared
            .id()
            .iter()
            .map(|i| i.abs())
            .filter(|&i| i > 0.0)
            .fold(f64::INFINITY, f64::min);
        ac::gm_over_id(&gradient(prepared.id(), prepared.vg()), prepared.id())
            .into_iter()
            .zip(prepared.id())
            .filter_map(|(e, &i)| {
                let magnitude = i.abs();
                let density = magnitude / self.geometry.w_um;
                (magnitude > floor * 100.0
                    && e.is_finite()
                    && e > 0.0
                    && e <= GM_ID_CEILING_V_INV
                    && density.is_finite()
                    && density > 0.0)
                    .then_some([e, density])
            })
            .collect()
    }

    pub fn set_geometry(&mut self, geometry: GeometryParams) -> Result<(), InputError> {
        let w = geometry.w_um * 1.0e-6;
        let l = geometry.l_um * 1.0e-6;
        if !(geometry.w_um.is_finite()
            && geometry.l_um.is_finite()
            && geometry.w_um > 0.0
            && geometry.l_um > 0.0
            && w.is_finite()
            && l.is_finite()
            && (w * l).is_finite()
            && w / l > 0.0
            && (w / l).is_finite())
        {
            return Err(InputError::InvalidGeometry);
        }
        if self.level62.manual
            && self
                .level62
                .fit
                .as_ref()
                .is_some_and(|fit| !fit.params.is_valid_for(geometry))
        {
            return Err(InputError::InvalidGeometry);
        }
        let level62 = if self.level62.manual {
            None
        } else {
            Some(
                self.rebuild_level62_preserving_retained_dibl(
                    geometry,
                    self.bias.v_ds,
                    &self.output_curves,
                )
                .map_err(|RetainedDiblError::NotApplied| InputError::RetainedDiblNotApplied)?,
            )
        };
        self.geometry = geometry;
        self.invalidate_analog_quality_for(FitModel::Level62);
        if self.level62.manual {
            self.recompute_model_r2(FitModel::Level62);
        } else if let Some(level62) = level62 {
            self.level62.fit = level62.map(|fit| fit.fit);
        }
        Ok(())
    }
    /// Update the gate-capacitance density used by charge/export projections.
    /// Cox has no DC-extraction dependency, so this deliberately preserves every
    /// fitted transfer/output parameter.
    pub fn set_cox(&mut self, cox: f64) -> Result<(), InputError> {
        if !(cox.is_finite() && cox >= 0.0) {
            return Err(InputError::InvalidBias);
        }
        self.bias.cox = cox;
        Ok(())
    }
    /// Set the user-owned transfer VDS and Cox. Series resistance is not an
    /// input-card field, so this deliberately preserves its existing value.
    pub fn set_bias(&mut self, v_ds: f64, cox: f64) -> Result<(), InputError> {
        if !(v_ds.is_finite() && v_ds > 0.0 && cox.is_finite() && cox >= 0.0) {
            return Err(InputError::InvalidBias);
        }
        if v_ds == self.bias.v_ds {
            return self.set_cox(cox);
        }
        let next_bias = BiasParams {
            v_ds,
            cox,
            ..self.bias
        };
        if let Some(output) = self.aostft.output {
            if map_h_fit_to_card(self.aostft.h_fit, output, next_bias).is_none() {
                return Err(InputError::InvalidAostftCardMapping);
            }
        }
        let level62 = if self.level62.manual {
            None
        } else {
            Some(
                self.rebuild_level62_preserving_retained_dibl(
                    self.geometry,
                    next_bias.v_ds,
                    &self.output_curves,
                )
                .map_err(|RetainedDiblError::NotApplied| InputError::RetainedDiblNotApplied)?,
            )
        };
        self.bias = next_bias;
        self.aostft.invalidate_card_fit();
        self.invalidate_analog_quality();
        if !self.aostft.manual {
            self.reset_aostft();
        }
        if let Some(level62) = level62 {
            self.level62.fit = level62.map(|fit| fit.fit);
        }
        if self.aostft.manual {
            self.recompute_model_r2(FitModel::Aostft);
        }
        if self.level62.manual {
            self.recompute_model_r2(FitModel::Level62);
        }
        Ok(())
    }
    pub fn set_cox_from_accumulation(&mut self, c_acc: f64) -> Result<f64, InputError> {
        let area = self.geometry.w_um * 1.0e-6 * self.geometry.l_um * 1.0e-6;
        if !(c_acc.is_finite() && c_acc > 0.0 && area.is_finite() && area > 0.0) {
            return Err(InputError::InvalidAccumulationCapacitance);
        }
        let cox = c_acc / area;
        if !cox.is_finite() {
            return Err(InputError::InvalidAccumulationCapacitance);
        }
        self.set_cox(cox)
            .map_err(|_| InputError::InvalidAccumulationCapacitance)?;
        Ok(cox)
    }

    pub fn replace_output(
        &mut self,
        curves: Vec<OutputCurve>,
    ) -> Result<OutputReplacement, OutputReplacementError> {
        let level62 = if self.level62.manual {
            None
        } else {
            match self.rebuild_level62_preserving_retained_dibl(
                self.geometry,
                self.bias.v_ds,
                &curves,
            ) {
                Ok(level62) => level62,
                Err(RetainedDiblError::NotApplied) => {
                    return Err(OutputReplacementError::RetainedDiblNotApplied {
                        rejected: curves,
                    });
                }
            }
        };
        let level62_output_fitted = level62.as_ref().is_some_and(|fit| fit.output_fitted);
        let extracted = extract_output(&curves, self.aostft.h_fit.vt, self.polarity);
        let displaced = std::mem::replace(&mut self.output_curves, curves);
        self.invalidate_analog_quality();
        let aostft = if self.aostft.manual {
            match extracted {
                Some(output)
                    if map_h_fit_to_card(self.aostft.h_fit, output, self.bias).is_some() =>
                {
                    self.aostft.output = Some(output);
                    self.aostft.invalidate_card_fit();
                    self.recompute_model_r2(FitModel::Aostft);
                    true
                }
                _ => {
                    self.aostft.output = None;
                    self.aostft.invalidate_card_fit();
                    self.recompute_model_r2(FitModel::Aostft);
                    false
                }
            }
        } else {
            let fitted = self.reset_aostft() && self.aostft.output.is_some();
            if !fitted {
                self.aostft.output = None;
                self.aostft.invalidate_card_fit();
                self.recompute_model_r2(FitModel::Aostft);
            }
            fitted
        };
        if !self.level62.manual {
            self.level62.fit = level62.map(|fit| fit.fit);
        }
        let outcome = if aostft || level62_output_fitted {
            OutputAttachOutcome::Fitted
        } else {
            OutputAttachOutcome::NoFit
        };
        Ok(OutputReplacement { outcome, displaced })
    }
    pub fn detach_output(&mut self) -> Result<Vec<OutputCurve>, DetachOutputError> {
        if self.output_curves.is_empty() {
            return Err(DetachOutputError::NoOutput);
        }
        let level62 = if self.level62.manual {
            None
        } else {
            Some(
                self.rebuild_level62_preserving_retained_dibl(self.geometry, self.bias.v_ds, &[])
                    .map_err(|RetainedDiblError::NotApplied| {
                        DetachOutputError::RetainedDiblNotApplied
                    })?,
            )
        };
        self.aostft.output = None;
        self.aostft.invalidate_card_fit();
        let curves = std::mem::take(&mut self.output_curves);
        self.invalidate_analog_quality();
        if !self.aostft.manual {
            self.reset_aostft();
        }
        if let Some(level62) = level62 {
            self.level62.fit = level62.map(|fit| fit.fit);
        }
        if self.aostft.manual {
            self.recompute_model_r2(FitModel::Aostft);
        }
        Ok(curves)
    }

    pub fn replace_second_transfer(
        &mut self,
        second: SecondTransfer,
    ) -> Result<DiblReplacement, DiblReplacementError> {
        if second.vg.len() != second.id_abs.len()
            || second.vg.len() < 10
            || !second
                .vg
                .iter()
                .chain(&second.id_abs)
                .all(|sample| sample.is_finite())
            || !second.v_ds.is_finite()
            || second.v_ds.abs() < 1.0e-6
        {
            return Err(DiblReplacementError {
                reason: DiblError::InvalidSecondTransfer,
                rejected: second,
            });
        }
        if self.level62.manual {
            return Err(DiblReplacementError {
                reason: DiblError::Level62Manual,
                rejected: second,
            });
        }
        if self.level62.fit.is_none() {
            return Err(DiblReplacementError {
                reason: DiblError::Level62Unavailable,
                rejected: second,
            });
        }
        let v1 = self.bias.v_ds;
        let v2 = second.v_ds.abs();
        if (v2 - v1).abs() < 0.25 * v1.max(v2) {
            return Err(DiblReplacementError {
                reason: DiblError::BiasTooClose,
                rejected: second,
            });
        }
        let second = DiblTransfer {
            vg: second.vg,
            id: second.id_abs,
            v_ds: second.v_ds,
        };
        let Some(rebuild) = self.rebuild_level62(
            self.geometry,
            self.bias.v_ds,
            &self.output_curves,
            Some(&second),
        ) else {
            return Err(DiblReplacementError {
                reason: DiblError::Level62Unavailable,
                rejected: second.into(),
            });
        };
        if !rebuild.dibl_fitted {
            return Err(DiblReplacementError {
                reason: DiblError::NoImprovement,
                rejected: second.into(),
            });
        }
        let at = rebuild.fit.params.at;
        self.level62.fit = Some(rebuild.fit);
        let displaced = self.dibl_transfer.replace(second).map(SecondTransfer::from);
        self.invalidate_analog_quality_for(FitModel::Level62);
        Ok(DiblReplacement { at, displaced })
    }
    pub fn detach_second_transfer(&mut self) -> Result<SecondTransfer, DetachDiblError> {
        if self.dibl_transfer.is_none() {
            return Err(DetachDiblError::NoSecondTransfer);
        }
        let rebuilt = if self.level62.manual {
            None
        } else {
            Some(
                self.rebuild_level62(self.geometry, self.bias.v_ds, &self.output_curves, None)
                    .ok_or(DetachDiblError::CannotRestoreLevel62Fit)?,
            )
        };
        let second = self
            .dibl_transfer
            .take()
            .expect("the attached transfer was checked above");
        if let Some(rebuilt) = rebuilt {
            self.level62.fit = Some(rebuilt.fit);
        }
        self.invalidate_analog_quality_for(FitModel::Level62);
        Ok(second.into())
    }
    pub fn set_aostft_fit(&mut self, params: ModelParams) -> Result<(), EditError> {
        if !(params.vt.is_finite() && params.gamma.is_finite() && params.k.is_finite()) {
            return Err(EditError::NonFiniteAostftFit);
        }
        let card_is_active = self.aostft.output.is_some();
        let gamma_is_physical = if card_is_active {
            params.gamma >= -1.0
        } else {
            params.gamma > -1.0
        };
        if !(gamma_is_physical && params.k > 0.0) {
            return Err(EditError::InvalidAostftFit);
        }
        let candidate = AboveThresholdFit {
            vt: params.vt,
            gamma: params.gamma,
            k: params.k,
            ..*self.aostft_fit()
        };
        let h_fit = match self.aostft.output {
            Some(output) => map_card_fit_to_h(candidate, output, self.bias)
                .ok_or(EditError::InvalidAostftCardMapping)?,
            None => candidate,
        };
        self.aostft.h_fit = h_fit;
        self.aostft.invalidate_card_fit();
        self.aostft.manual = true;
        self.recompute_model_r2(FitModel::Aostft);
        Ok(())
    }
    pub fn set_aostft_subthreshold(&mut self, params: SubthresholdParams) -> Result<(), EditError> {
        if !(params.ss_v_dec.is_finite()
            && params.ss_v_dec > 0.0
            && params.ioff.is_finite()
            && params.ioff > 0.0)
        {
            return Err(EditError::InvalidSubthreshold);
        }
        self.aostft.subthreshold = Some(params);
        self.aostft.manual = true;
        self.recompute_model_r2(FitModel::Aostft);
        Ok(())
    }
    pub fn set_aostft_output(&mut self, params: OutputParams) -> Result<(), EditError> {
        if !(params.alpha_sat.is_finite()
            && params.alpha_sat > 0.0
            && params.lambda.is_finite()
            && params.lambda >= 0.0
            && params.m.is_finite()
            && params.m > 0.0)
        {
            return Err(EditError::InvalidOutput);
        }
        map_h_fit_to_card(self.aostft.h_fit, params, self.bias)
            .ok_or(EditError::InvalidAostftCardMapping)?;
        self.aostft.output = Some(params);
        self.aostft.invalidate_card_fit();
        self.aostft.manual = true;
        self.recompute_model_r2(FitModel::Aostft);
        Ok(())
    }
    pub fn set_level62_params(&mut self, params: Level62Params) -> Result<(), EditError> {
        if self.level62.fit.is_none() {
            return Err(EditError::NoLevel62Fit);
        }
        if !params.is_valid_for(self.geometry) {
            return Err(EditError::InvalidLevel62Params);
        }
        let fit = self.level62.fit.as_mut().unwrap();
        fit.params = params;
        self.level62.manual = true;
        self.recompute_model_r2(FitModel::Level62);
        Ok(())
    }
    pub fn reset_autofit(&mut self, model: FitModel) -> Result<(), RefitError> {
        let ok = match model {
            FitModel::Aostft => self.reset_aostft(),
            FitModel::Level62 => {
                match self.rebuild_level62_preserving_retained_dibl(
                    self.geometry,
                    self.bias.v_ds,
                    &self.output_curves,
                ) {
                    Ok(Some(rebuild)) => {
                        self.level62.fit = Some(rebuild.fit);
                        true
                    }
                    Ok(None) => false,
                    Err(RetainedDiblError::NotApplied) => {
                        return Err(RefitError::RetainedDiblNotApplied);
                    }
                }
            }
        };
        if !ok {
            return Err(RefitError::ExtractionFailed);
        }
        match model {
            FitModel::Aostft => self.aostft.manual = false,
            FitModel::Level62 => self.level62.manual = false,
        }
        self.invalidate_analog_quality_for(model);
        Ok(())
    }

    fn reset_aostft(&mut self) -> bool {
        let Some(prepared) = prepare_transfer(&self.vgs, &self.id) else {
            return false;
        };
        let Some(mut fit) = extract_above_threshold(prepared.vg(), prepared.id()) else {
            return false;
        };
        let polarity = prepared.polarity();
        fit.vt = polarity.map_vg(fit.vt);
        let sub = extract_subthreshold(prepared.vg(), prepared.id());
        let output = if self.output_curves.is_empty() {
            None
        } else {
            extract_output(&self.output_curves, fit.vt, self.polarity).map(|seed| {
                aostft_output::refine_output(
                    seed,
                    &self.output_curves,
                    (&self.vgs, &self.id),
                    fit,
                    sub.unwrap_or_else(SubthresholdParams::card_defaults),
                    self.bias,
                    self.polarity,
                )
                .unwrap_or(seed)
            })
        };
        if let Some(output) = output {
            if map_h_fit_to_card(fit, output, self.bias).is_none() {
                return false;
            }
        }
        self.aostft.h_fit = fit;
        self.aostft.subthreshold = sub;
        self.aostft.output = output;
        self.aostft.invalidate_card_fit();
        self.recompute_model_r2(FitModel::Aostft);
        true
    }
    /// Project the current AOSTFT state into the strict finite-drain card
    /// representation without changing the stored H-fit representation.
    fn aostft_card_projection(&self) -> Option<AostftCardProjection> {
        self.aostft.card_projection(self.bias)
    }
    /// Build one complete automatic Level 62 candidate without mutating the
    /// device. Every caller therefore applies the same base → output → DIBL
    /// order, and can commit or reject the whole candidate atomically.
    fn rebuild_level62_preserving_retained_dibl(
        &self,
        geometry: GeometryParams,
        v_ds: f64,
        output_curves: &[OutputCurve],
    ) -> Result<Option<Level62Rebuild>, RetainedDiblError> {
        let rebuild =
            self.rebuild_level62(geometry, v_ds, output_curves, self.dibl_transfer.as_ref());
        if self.dibl_transfer.is_some()
            && !rebuild
                .as_ref()
                .is_some_and(|candidate| candidate.dibl_fitted)
        {
            return Err(RetainedDiblError::NotApplied);
        }
        Ok(rebuild)
    }
    fn rebuild_level62(
        &self,
        geometry: GeometryParams,
        v_ds: f64,
        output_curves: &[OutputCurve],
        dibl_transfer: Option<&DiblTransfer>,
    ) -> Option<Level62Rebuild> {
        let mut fit = fit_level62(&self.vgs, &self.id, geometry, v_ds)?;
        let output = refine_level62_output(
            fit.params,
            output_curves,
            geometry,
            fit.params.tnom_k,
            v_ds,
            (&self.vgs, &self.id),
            self.polarity,
        )
        .filter(|params| params.is_valid_for(geometry));
        let output_fitted = output.is_some();
        if let Some(params) = output {
            fit.params = params;
        }

        let dibl = dibl_transfer.and_then(|second| {
            let s = self.polarity.sign();
            let transfers = [
                (&self.vgs[..], &self.id[..], s * v_ds),
                (&second.vg[..], &second.id[..], second.v_ds),
            ];
            refine_level62_dibl(
                fit.params,
                &transfers,
                geometry,
                fit.params.tnom_k,
                self.polarity,
            )
            .filter(|params| params.is_valid_for(geometry))
        });
        let dibl_fitted = dibl.is_some();
        if let Some(params) = dibl {
            fit.params = params;
        }
        fit.r2 = self.level62_log_r2(&fit.params, geometry, v_ds);
        Some(Level62Rebuild {
            fit,
            output_fitted,
            dibl_fitted,
        })
    }
    fn level62_log_r2(&self, params: &Level62Params, geometry: GeometryParams, v_ds: f64) -> f64 {
        let vg: Vec<_> = self.vgs.iter().map(|&v| self.polarity.map_vg(v)).collect();
        overlay_log_r2(
            &level62_transfer(params, geometry, params.tnom_k, &vg, v_ds),
            &self.id,
        )
    }
    fn recompute_model_r2(&mut self, model: FitModel) {
        self.invalidate_analog_quality_for(model);
        let values: Vec<_> = self
            .modelled_points(model)
            .into_iter()
            .map(|p| p[1])
            .collect();
        let r2 = overlay_log_r2(&values, &self.id);
        match model {
            FitModel::Aostft => {
                self.aostft.h_fit.r2 = r2;
                self.aostft.invalidate_card_fit();
            }
            FitModel::Level62 => {
                if let Some(fit) = self.level62.fit.as_mut() {
                    fit.r2 = r2
                }
            }
        }
    }
    fn modelled_points(&self, model: FitModel) -> Vec<[f64; 2]> {
        match model {
            FitModel::Aostft => self.aostft_modelled_points(),
            FitModel::Level62 => self.level62_modelled_points(),
        }
    }
    fn overlay_points(&self, f: impl FnOnce(&[f64]) -> Vec<f64>) -> Vec<[f64; 2]> {
        let vg: Vec<_> = self.vgs.iter().map(|&v| self.polarity.map_vg(v)).collect();
        self.vgs.iter().zip(f(&vg)).map(|(&v, i)| [v, i]).collect()
    }
    fn aostft_modelled_points(&self) -> Vec<[f64; 2]> {
        let sub = self
            .aostft
            .subthreshold
            .unwrap_or_else(SubthresholdParams::card_defaults);
        let h_fit = self.aostft.h_fit;
        let card = self.aostft.output.map(|_| {
            self.aostft_card_projection()
                .expect("AOSTFT output mutations preserve a valid card projection")
        });
        let displayed_fit = card.map_or(h_fit, |projection| projection.fit);
        let vt = self.polarity.map_vg(displayed_fit.vt);
        self.overlay_points(|vg| match card {
            Some(projection) if self.bias.v_ds > 0.0 => vg
                .iter()
                .map(|&v| {
                    self.card_output_with_fit(
                        projection.fit,
                        projection.output,
                        v - vt,
                        &[self.bias.v_ds],
                    )[0]
                })
                .collect(),
            _ => unified_transfer(vt, h_fit.gamma, h_fit.k, &sub, vg),
        })
    }
    fn level62_modelled_points(&self) -> Vec<[f64; 2]> {
        let Some(fit) = &self.level62.fit else {
            return Vec::new();
        };
        self.overlay_points(|vg| {
            level62_transfer(
                &fit.params,
                self.geometry,
                fit.params.tnom_k,
                vg,
                self.bias.v_ds,
            )
        })
    }
    fn card_output_with_fit(
        &self,
        fit: AboveThresholdFit,
        p: OutputParams,
        vov: f64,
        vd: &[f64],
    ) -> Vec<f64> {
        output_card_current(
            fit.k / self.bias.v_ds,
            fit.gamma,
            self.bias.r,
            &p,
            &self
                .aostft
                .subthreshold
                .unwrap_or_else(SubthresholdParams::card_defaults),
            vov,
            vd,
        )
    }
    fn invalidate_analog_quality(&self) {
        self.aostft.analog_quality_cache.set(None);
        self.level62.analog_quality_cache.set(None);
    }
    fn invalidate_analog_quality_for(&self, model: FitModel) {
        match model {
            FitModel::Aostft => self.aostft.analog_quality_cache.set(None),
            FitModel::Level62 => self.level62.analog_quality_cache.set(None),
        }
    }
}

fn map_h_fit_to_card(
    mut fit: AboveThresholdFit,
    output: OutputParams,
    bias: BiasParams,
) -> Option<AboveThresholdFit> {
    let gamma = fit.gamma - 1.0;
    let scale = output.alpha_sat * (1.0 + output.lambda * bias.v_ds);
    if !(gamma.is_finite()
        && gamma >= -1.0
        && scale.is_finite()
        && scale > 0.0
        && bias.v_ds.is_finite()
        && bias.v_ds > 0.0)
    {
        return None;
    }
    fit.gamma = gamma;
    fit.k *= bias.v_ds / scale;
    (fit.k.is_finite() && fit.k > 0.0).then_some(fit)
}
fn map_card_fit_to_h(
    mut fit: AboveThresholdFit,
    output: OutputParams,
    bias: BiasParams,
) -> Option<AboveThresholdFit> {
    let scale = output.alpha_sat * (1.0 + output.lambda * bias.v_ds);
    if !(scale.is_finite() && scale > 0.0 && bias.v_ds.is_finite() && bias.v_ds > 0.0) {
        return None;
    }
    fit.gamma += 1.0;
    fit.k *= scale / bias.v_ds;
    (fit.gamma.is_finite() && fit.gamma >= 0.0 && fit.k.is_finite() && fit.k > 0.0).then_some(fit)
}
fn overlay_log_r2(modelled: &[f64], measured: &[f64]) -> f64 {
    let n = modelled.len().min(measured.len());
    if n < 2 {
        return f64::NAN;
    }
    let log = |v: f64| v.max(1e-30).log10();
    let ys: Vec<_> = measured[..n].iter().map(|&v| log(v)).collect();
    let mean = ys.iter().sum::<f64>() / n as f64;
    let total: f64 = ys.iter().map(|v| (v - mean).powi(2)).sum();
    if total <= 0.0 {
        return f64::NAN;
    }
    1.0 - modelled[..n]
        .iter()
        .zip(&ys)
        .map(|(&m, &y)| (log(m) - y).powi(2))
        .sum::<f64>()
        / total
}
fn saturation_default_vds(vgs: &[f64], polarity: Polarity) -> f64 {
    vgs.iter()
        .map(|&v| polarity.map_vg(v))
        .fold(f64::NEG_INFINITY, f64::max)
        .max(BiasParams::default().v_ds)
}
fn fit_level62(vg: &[f64], id: &[f64], geom: GeometryParams, vds: f64) -> Option<Level62Fit> {
    let fit = extract_level62(vg, id, geom, vds, LEVEL62_TNOM_K, Level62Params::ltps())?;
    fit.params.is_valid_for(geom).then_some(fit)
}
#[cfg(test)]
type FixtureCache<T> = OnceLock<Mutex<T>>;

#[cfg(test)]
fn clone_fixture<T: Clone + 'static>(
    cache: &'static FixtureCache<T>,
    initialize: impl FnOnce() -> T,
) -> T {
    cache
        .get_or_init(|| Mutex::new(initialize()))
        .lock()
        .expect("fixture cache lock remains available")
        .clone()
}

#[cfg(test)]
#[path = "tests/fitted_device_lifecycle.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "tests/fitted_device_series.rs"]
mod series_tests;
