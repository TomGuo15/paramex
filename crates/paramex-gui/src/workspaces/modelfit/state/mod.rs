//! Transient Model Fit workspace state.
//!
//! [`FittedDevice`] owns measurement data and every derived compact-model result.
//! This module owns only GUI row state: selection, checkboxes, source metadata,
//! and pending-file pairing.

mod ingest_issues;
mod pairing;
mod refinement;

use std::path::{Path, PathBuf};

use super::models::FIT_MODELS;
#[cfg(test)]
use super::models::{AOSTFT_INDEX, LEVEL62_INDEX};
use paramex_core::modelfit::{
    DetachDiblError, DetachOutputError, DiblError, DiblReplacementError, EditError, FitModel,
    FittedDevice, GeometryParams, InputError, Level62Params, ModelParams, OutputAttachOutcome,
    OutputCurve, OutputParams, OutputReplacementError, RefitError, SecondTransfer,
    SubthresholdParams,
};
use paramex_core::shared::same_named_source;

use crate::workspaces::upsert_match_set;

pub use ingest_issues::{IngestIssues, IssueRow};
#[cfg(test)]
pub(crate) use refinement::DiblFit;
pub(crate) use refinement::OutputRefinementResult;
pub(crate) use refinement::{
    run_dibl_refinement, run_output_refinement, run_setup_refinement, DiblCommitReport, DiblImport,
    DiblIssue, DiblIssueKind, DiblRefinementMode, DiblRefinementPlan, DiblRefinementPurpose,
    DiblRefinementRecovery, DiblRefinementResult, OutputImport, OutputIssue, OutputRefinementPlan,
    OutputRefinementPurpose, OutputRefinementRecovery, SetupCommitOutcome, SetupOperation,
    SetupRefinementError, SetupRefinementPlan, SetupRefinementPurpose, SetupRefinementResult,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceId(u64);

impl DeviceId {
    #[cfg(test)]
    pub(crate) fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScientificRevision(u64);

impl ScientificRevision {
    #[cfg(test)]
    pub(crate) fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceToken {
    id: DeviceId,
    revision: ScientificRevision,
}

/// Visible provenance for the primary transfer measurement that created one
/// Model Fit device row. This role is mandatory and cannot be interchanged with
/// output or DIBL attachment sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimaryTransferSource {
    name: String,
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryTransferSourceError {
    EmptyName,
}

impl PrimaryTransferSource {
    pub fn new(
        name: impl Into<String>,
        path: Option<PathBuf>,
    ) -> Result<Self, PrimaryTransferSourceError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(PrimaryTransferSourceError::EmptyName);
        }
        Ok(Self { name, path })
    }

    pub(crate) fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| {
                let displayed = path.display().to_string();
                if displayed.trim().is_empty() {
                    "(unnamed file)".to_owned()
                } else {
                    displayed
                }
            });
        Self {
            name,
            path: Some(path.to_path_buf()),
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Visible provenance for output curves already owned by an installed
/// [`FittedDevice`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputSource {
    name: String,
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputSourceError {
    EmptyName,
}

impl OutputSource {
    pub fn new(name: impl Into<String>, path: Option<PathBuf>) -> Result<Self, OutputSourceError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(OutputSourceError::EmptyName);
        }
        Ok(Self { name, path })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Visible provenance for the second transfer measurement currently refining a
/// fitted device's Level 62 DIBL terms. This is deliberately a distinct type
/// from [`OutputSource`] so the two attachment roles cannot be interchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiblSource {
    name: String,
    path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiblSourceError {
    EmptyName,
}

impl DiblSource {
    pub fn new(name: impl Into<String>, path: Option<PathBuf>) -> Result<Self, DiblSourceError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DiblSourceError::EmptyName);
        }
        Ok(Self { name, path })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceInstallError {
    PrimarySourceNameMismatch,
    OutputSourceRequired,
    OutputSourceWithoutCurves,
    DiblSourceRequired,
    DiblSourceWithoutSecondTransfer,
}

#[must_use = "device admission may report an already-loaded primary source"]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceInstallOutcome {
    Installed,
    AlreadyLoaded,
}

/// One valid scientific value and the GUI provenance for its primary and
/// attached measurements. The fields stay private so scientific inputs and
/// visible provenance cannot be updated independently.
#[derive(Clone)]
struct DeviceScience {
    device: FittedDevice,
    primary_source: PrimaryTransferSource,
    output_source: Option<OutputSource>,
    dibl_source: Option<DiblSource>,
}

struct DetachedOutput {
    source: OutputSource,
    curves: Vec<OutputCurve>,
}

struct DetachedDibl {
    source: DiblSource,
    second: SecondTransfer,
}

struct OutputScienceReplacement {
    science: DeviceScience,
    outcome: OutputAttachOutcome,
    displaced: Option<DetachedOutput>,
}

struct RejectedOutput {
    source: OutputSource,
    curves: Vec<OutputCurve>,
}

#[derive(Debug)]
struct RejectedDibl {
    source: DiblSource,
    second: SecondTransfer,
    reason: DiblError,
}

#[must_use = "displaced DIBL science must be handled"]
struct DiblScienceReplacement {
    science: DeviceScience,
    at: f64,
    displaced: Option<DetachedDibl>,
}

impl DeviceScience {
    fn new(
        device: FittedDevice,
        primary_source: PrimaryTransferSource,
        output_source: Option<OutputSource>,
        dibl_source: Option<DiblSource>,
    ) -> Result<Self, DeviceInstallError> {
        if device.name() != primary_source.name() {
            return Err(DeviceInstallError::PrimarySourceNameMismatch);
        }
        match (device.has_output_curves(), output_source.is_some()) {
            (true, false) => return Err(DeviceInstallError::OutputSourceRequired),
            (false, true) => return Err(DeviceInstallError::OutputSourceWithoutCurves),
            _ => {}
        }
        match (device.has_second_transfer(), dibl_source.is_some()) {
            (true, false) => return Err(DeviceInstallError::DiblSourceRequired),
            (false, true) => return Err(DeviceInstallError::DiblSourceWithoutSecondTransfer),
            _ => {}
        }
        Ok(Self {
            device,
            primary_source,
            output_source,
            dibl_source,
        })
    }

    fn device(&self) -> &FittedDevice {
        &self.device
    }

    fn primary_source(&self) -> &PrimaryTransferSource {
        &self.primary_source
    }

    fn output_source(&self) -> Option<&OutputSource> {
        self.output_source.as_ref()
    }

    fn dibl_source(&self) -> Option<&DiblSource> {
        self.dibl_source.as_ref()
    }

    fn replacing_output(
        &self,
        source: OutputSource,
        curves: Vec<OutputCurve>,
    ) -> Result<OutputScienceReplacement, RejectedOutput> {
        let mut science = self.clone();
        let old_source = science.output_source.take();
        let replacement = science.device.replace_output(curves).map_err(
            |OutputReplacementError::RetainedDiblNotApplied { rejected }| RejectedOutput {
                source: source.clone(),
                curves: rejected,
            },
        )?;
        let displaced = old_source
            .filter(|old_source| !same_output_source(old_source, &source))
            .map(|source| DetachedOutput {
                source,
                curves: replacement.displaced,
            });
        science.output_source = science.device.has_output_curves().then_some(source);
        Ok(OutputScienceReplacement {
            science,
            outcome: replacement.outcome,
            displaced,
        })
    }

    fn without_output(&self) -> Result<(Self, DetachedOutput), DetachOutputError> {
        let mut science = self.clone();
        let curves = science.device.detach_output()?;
        let source = science
            .output_source
            .take()
            .expect("validated output science always has provenance");
        Ok((science, DetachedOutput { source, curves }))
    }

    fn replacing_second_transfer(
        &self,
        source: DiblSource,
        second: SecondTransfer,
    ) -> Result<DiblScienceReplacement, RejectedDibl> {
        let mut science = self.clone();
        let old_source = science.dibl_source.take();
        let replacement = match science.device.replace_second_transfer(second) {
            Ok(replacement) => replacement,
            Err(DiblReplacementError { reason, rejected }) => {
                return Err(RejectedDibl {
                    source,
                    second: rejected,
                    reason,
                });
            }
        };
        let displaced = match (old_source, replacement.displaced) {
            (Some(old_source), Some(second)) => (!same_dibl_source(&old_source, &source))
                .then_some(DetachedDibl {
                    source: old_source,
                    second,
                }),
            (None, None) => None,
            _ => unreachable!("validated DIBL science and core measurement ownership agree"),
        };
        science.dibl_source = Some(source);
        Ok(DiblScienceReplacement {
            science,
            at: replacement.at,
            displaced,
        })
    }

    fn without_second_transfer(&self) -> Result<(Self, DetachedDibl), DetachDiblError> {
        let mut science = self.clone();
        let second = science.device.detach_second_transfer()?;
        let source = science
            .dibl_source
            .take()
            .expect("validated DIBL science always has provenance");
        Ok((science, DetachedDibl { source, second }))
    }

    fn set_geometry(&mut self, geometry: GeometryParams) -> Result<(), InputError> {
        self.device.set_geometry(geometry)
    }

    fn set_drain_bias(&mut self, v_ds: f64) -> Result<(), InputError> {
        self.device.set_bias(v_ds, self.device.bias().cox)
    }

    fn reset_autofit(&mut self, model: FitModel) -> Result<(), RefitError> {
        self.device.reset_autofit(model)
    }
}

/// One GUI device row: stable identity/revision, checkbox state, and exactly one
/// validated scientific value with its attachment provenance.
pub struct DeviceEntry {
    id: DeviceId,
    revision: ScientificRevision,
    checked: bool,
    science: DeviceScience,
}

impl DeviceEntry {
    fn new(id: DeviceId, science: DeviceScience) -> Self {
        Self {
            id,
            revision: ScientificRevision::default(),
            checked: false,
            science,
        }
    }

    fn science(&self) -> &DeviceScience {
        &self.science
    }

    pub fn device(&self) -> &FittedDevice {
        self.science.device()
    }

    pub fn id(&self) -> DeviceId {
        self.id
    }

    #[cfg(test)]
    pub(crate) fn revision(&self) -> ScientificRevision {
        self.revision
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn transfer_name(&self) -> &str {
        self.science.primary_source().name()
    }

    pub fn transfer_source_path(&self) -> Option<&Path> {
        self.science.primary_source().path()
    }

    pub fn output_name(&self) -> Option<&str> {
        self.science.output_source().map(OutputSource::name)
    }

    pub fn output_source_path(&self) -> Option<&Path> {
        self.science.output_source().and_then(OutputSource::path)
    }

    pub fn dibl_name(&self) -> Option<&str> {
        self.science.dibl_source().map(DiblSource::name)
    }

    pub fn dibl_source_path(&self) -> Option<&Path> {
        self.science.dibl_source().and_then(DiblSource::path)
    }

    fn token(&self) -> DeviceToken {
        DeviceToken {
            id: self.id,
            revision: self.revision,
        }
    }

    fn bump_revision(&mut self) {
        self.revision.0 = self
            .revision
            .0
            .checked_add(1)
            .expect("scientific revision space exhausted");
    }

    fn commit_science(&mut self, science: DeviceScience) {
        self.science = science;
        self.bump_revision();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingOutputReason {
    NoMatch,
    Ambiguous,
    Detached,
    DeviceChanged,
    DiblConflict,
    WorkerFailed,
}

impl PendingOutputReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoMatch => "No match",
            Self::Ambiguous => "Ambiguous",
            Self::Detached => "Detached",
            Self::DeviceChanged => "Device changed",
            Self::DiblConflict => "DIBL conflict",
            Self::WorkerFailed => "Worker failed",
        }
    }
}

#[derive(Clone)]
pub struct PendingOutput {
    source: OutputSource,
    curves: Vec<OutputCurve>,
    reason: PendingOutputReason,
}

impl PendingOutput {
    pub fn name(&self) -> &str {
        self.source.name()
    }

    pub fn reason(&self) -> PendingOutputReason {
        self.reason
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source.path()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingDiblReason {
    NoMatch,
    Ambiguous,
    NoFit,
    Detached,
    DeviceChanged,
    WorkerFailed,
}

impl PendingDiblReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoMatch => "No match",
            Self::Ambiguous => "Ambiguous",
            Self::NoFit => "No fit",
            Self::Detached => "Detached",
            Self::DeviceChanged => "Device changed",
            Self::WorkerFailed => "Worker failed",
        }
    }
}

#[derive(Clone)]
pub struct PendingDibl {
    source: DiblSource,
    second: SecondTransfer,
    reason: PendingDiblReason,
}

impl PendingDibl {
    pub fn name(&self) -> &str {
        self.source.name()
    }

    pub fn reason(&self) -> PendingDiblReason {
        self.reason
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source.path()
    }
}

fn same_output_source(a: &OutputSource, b: &OutputSource) -> bool {
    same_named_source(a.name(), a.path(), b.name(), b.path())
}

fn same_dibl_source(a: &DiblSource, b: &DiblSource) -> bool {
    same_named_source(a.name(), a.path(), b.name(), b.path())
}

fn same_primary_source(a: &PrimaryTransferSource, b: &PrimaryTransferSource) -> bool {
    same_named_source(a.name(), a.path(), b.name(), b.path())
}

/// A selected-row mutation either has no target or preserves the typed rejection
/// returned by the core fitted-device lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedMutationError {
    NoDeviceSelected,
    Input(InputError),
    Edit(EditError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CvCommitError {
    DeviceChanged,
    Mutation(SelectedMutationError),
}

#[derive(Default)]
pub struct ModelFitState {
    devices: Vec<DeviceEntry>,
    pending_outputs: Vec<PendingOutput>,
    pending_dibls: Vec<PendingDibl>,
    selected: Option<usize>,
    selected_model: usize,
    next_device_id: u64,
}

impl ModelFitState {
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn devices(&self) -> &[DeviceEntry] {
        &self.devices
    }

    pub fn device_count(&self) -> usize {
        self.devices.len()
    }

    pub fn pending_outputs(&self) -> &[PendingOutput] {
        &self.pending_outputs
    }

    pub fn pending_dibls(&self) -> &[PendingDibl] {
        &self.pending_dibls
    }

    pub fn has_checked_devices(&self) -> bool {
        self.devices.iter().any(DeviceEntry::is_checked)
    }

    pub fn has_unchecked_devices(&self) -> bool {
        self.devices.iter().any(|entry| !entry.is_checked())
    }

    pub fn set_device_checked(&mut self, idx: usize, checked: bool) -> bool {
        let Some(entry) = self.devices.get_mut(idx) else {
            return false;
        };
        entry.checked = checked;
        true
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, idx: usize) {
        if idx < self.devices.len() {
            self.selected = Some(idx);
        }
    }

    pub fn selected_model(&self) -> usize {
        self.selected_model
    }

    pub fn selected_fit_model(&self) -> FitModel {
        FIT_MODELS[self.selected_model].fit_model
    }

    pub fn selected_model_is_level62(&self) -> bool {
        self.selected_fit_model() == FitModel::Level62
    }

    pub fn selected_entry(&self) -> Option<&DeviceEntry> {
        self.selected.and_then(|idx| self.devices.get(idx))
    }

    pub fn selected_device_id(&self) -> Option<DeviceId> {
        self.selected_entry().map(DeviceEntry::id)
    }

    pub fn selected_token(&self) -> Option<DeviceToken> {
        self.selected_entry().map(DeviceEntry::token)
    }

    pub fn set_selected_model(&mut self, idx: usize) -> bool {
        if FIT_MODELS.get(idx).is_none() {
            return false;
        }
        self.selected_model = idx;
        true
    }

    /// Install science already completed off the UI thread.
    ///
    /// Output curves and their provenance are one invariant: a device with output
    /// curves requires a source, while a transfer-only device rejects one.
    pub fn install_fitted_device(
        &mut self,
        device: FittedDevice,
        primary_source: PrimaryTransferSource,
        output_source: Option<OutputSource>,
    ) -> Result<DeviceInstallOutcome, DeviceInstallError> {
        let science = DeviceScience::new(device, primary_source, output_source, None)?;
        if self.devices.iter().any(|entry| {
            same_primary_source(entry.science.primary_source(), science.primary_source())
        }) {
            return Ok(DeviceInstallOutcome::AlreadyLoaded);
        }
        let id = DeviceId(self.next_device_id);
        self.next_device_id = self
            .next_device_id
            .checked_add(1)
            .expect("Model Fit device identity space exhausted");
        self.devices.push(DeviceEntry::new(id, science));
        if self.selected.is_none() {
            self.selected = Some(self.devices.len() - 1);
        }
        Ok(DeviceInstallOutcome::Installed)
    }

    pub fn clear(&mut self) {
        self.devices.clear();
        self.pending_outputs.clear();
        self.pending_dibls.clear();
        self.selected = None;
    }

    #[cfg(test)]
    pub(crate) fn remove_device(&mut self, idx: usize) -> bool {
        if idx >= self.devices.len() {
            return false;
        }
        let mut remove = vec![false; self.devices.len()];
        remove[idx] = true;
        self.remove_devices(&remove) == 1
    }

    pub fn remove_selected_or_checked(&mut self) -> usize {
        let has_checked = self.has_checked_devices();
        let remove: Vec<bool> = self
            .devices
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                if has_checked {
                    entry.is_checked()
                } else {
                    self.selected == Some(idx)
                }
            })
            .collect();
        self.remove_devices(&remove)
    }

    pub fn keep_checked_devices(&mut self) -> Option<usize> {
        if !self.has_checked_devices() {
            return None;
        }
        let remove: Vec<bool> = self
            .devices
            .iter()
            .map(|entry| !entry.is_checked())
            .collect();
        Some(self.remove_devices(&remove))
    }

    fn remove_devices(&mut self, remove: &[bool]) -> usize {
        let removed = remove.iter().filter(|&&remove| remove).count();
        if removed == 0 {
            return 0;
        }
        let old_selected = self.selected;
        let selected = old_selected.and_then(|idx| {
            (!remove[idx]).then(|| remove[..idx].iter().filter(|&&remove| !remove).count())
        });
        let mut idx = 0;
        self.devices.retain(|_| {
            let keep = !remove[idx];
            idx += 1;
            keep
        });
        self.selected = if self.devices.is_empty() {
            None
        } else {
            selected.or(Some(0))
        };
        removed
    }

    pub fn remove_pending_output(&mut self, idx: usize) -> bool {
        if idx >= self.pending_outputs.len() {
            return false;
        }
        self.pending_outputs.remove(idx);
        true
    }

    pub fn remove_pending_dibl(&mut self, idx: usize) -> bool {
        if idx >= self.pending_dibls.len() {
            return false;
        }
        self.pending_dibls.remove(idx);
        true
    }

    fn add_pending_output(
        &mut self,
        source: OutputSource,
        curves: Vec<OutputCurve>,
        reason: PendingOutputReason,
    ) {
        let pending = PendingOutput {
            source,
            curves,
            reason,
        };
        upsert_match_set(&mut self.pending_outputs, pending, |old, incoming| {
            same_output_source(&old.source, &incoming.source)
        });
    }

    fn add_pending_dibl(
        &mut self,
        source: DiblSource,
        second: SecondTransfer,
        reason: PendingDiblReason,
    ) {
        let pending = PendingDibl {
            source,
            second,
            reason,
        };
        upsert_match_set(&mut self.pending_dibls, pending, |old, incoming| {
            same_dibl_source(&old.source, &incoming.source)
        });
    }

    pub fn set_selected_cox(&mut self, cox: f64) -> Result<(), SelectedMutationError> {
        let entry = self.selected_entry_mut_or_error()?;
        if entry.science.device.bias().cox == cox {
            return Ok(());
        }
        entry
            .science
            .device
            .set_cox(cox)
            .map_err(SelectedMutationError::Input)?;
        entry.bump_revision();
        Ok(())
    }

    pub fn commit_cox_from_cv(
        &mut self,
        target: Option<DeviceToken>,
        c_acc: f64,
    ) -> Result<f64, CvCommitError> {
        let Some(target) = target else {
            return Err(CvCommitError::Mutation(
                SelectedMutationError::NoDeviceSelected,
            ));
        };
        let entry = self
            .devices
            .iter_mut()
            .find(|entry| entry.id == target.id)
            .filter(|entry| entry.revision == target.revision)
            .ok_or(CvCommitError::DeviceChanged)?;
        let old_cox = entry.science.device.bias().cox;
        let cox = entry
            .science
            .device
            .set_cox_from_accumulation(c_acc)
            .map_err(SelectedMutationError::Input)
            .map_err(CvCommitError::Mutation)?;
        if cox != old_cox {
            entry.bump_revision();
        }
        Ok(cox)
    }

    pub fn is_selected_manual(&self, model: FitModel) -> bool {
        self.selected_entry()
            .is_some_and(|entry| entry.science.device.model(model).is_manual())
    }

    pub fn set_selected_fit(
        &mut self,
        vt: f64,
        gamma: f64,
        k: f64,
    ) -> Result<(), SelectedMutationError> {
        let entry = self.selected_entry_mut_or_error()?;
        entry
            .science
            .device
            .set_aostft_fit(ModelParams { vt, gamma, k })
            .map_err(SelectedMutationError::Edit)?;
        entry.bump_revision();
        Ok(())
    }

    pub fn set_selected_subthreshold(
        &mut self,
        ss_v_dec: f64,
        ioff: f64,
    ) -> Result<(), SelectedMutationError> {
        let entry = self.selected_entry_mut_or_error()?;
        entry
            .science
            .device
            .set_aostft_subthreshold(SubthresholdParams { ss_v_dec, ioff })
            .map_err(SelectedMutationError::Edit)?;
        entry.bump_revision();
        Ok(())
    }

    pub fn set_selected_output(
        &mut self,
        alpha_sat: f64,
        lambda: f64,
        m: f64,
    ) -> Result<(), SelectedMutationError> {
        let entry = self.selected_entry_mut_or_error()?;
        entry
            .science
            .device
            .set_aostft_output(OutputParams {
                alpha_sat,
                lambda,
                m,
            })
            .map_err(SelectedMutationError::Edit)?;
        entry.bump_revision();
        Ok(())
    }

    pub fn set_selected_level62_params(
        &mut self,
        params: Level62Params,
    ) -> Result<(), SelectedMutationError> {
        let entry = self.selected_entry_mut_or_error()?;
        entry
            .science
            .device
            .set_level62_params(params)
            .map_err(SelectedMutationError::Edit)?;
        entry.bump_revision();
        Ok(())
    }

    fn selected_entry_mut(&mut self) -> Option<&mut DeviceEntry> {
        self.selected.and_then(|idx| self.devices.get_mut(idx))
    }

    fn selected_entry_mut_or_error(&mut self) -> Result<&mut DeviceEntry, SelectedMutationError> {
        self.selected_entry_mut()
            .ok_or(SelectedMutationError::NoDeviceSelected)
    }
}

fn dibl_error_message(error: DiblError, primary_vds: f64, second_vds: f64) -> String {
    match error {
        DiblError::InvalidSecondTransfer => {
            "the second transfer does not contain a valid paired V_G / |I_D| sweep".to_owned()
        }
        DiblError::Level62Manual => {
            "Level 62 is in manual mode for this device — reset it to auto-fit first".to_owned()
        }
        DiblError::Level62Unavailable => {
            "the selected device has no Level 62 fit to refine".to_owned()
        }
        DiblError::BiasTooClose => format!(
            "the second transfer's V_DS ({second_vds:.3} V) is too close to the device's \
             V_DS ({primary_vds:.3} V) — DIBL needs two distinct biases"
        ),
        DiblError::NoImprovement => {
            "the pair did not improve the fit — check that both sweeps are the same device and polarity"
                .to_owned()
        }
    }
}

#[cfg(test)]
pub(crate) fn synthetic_transfer(params: &ModelParams, vgs: &[f64]) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| {
            let overdrive = vg - params.vt;
            if overdrive > 0.0 {
                params.k * overdrive.powf(1.0 + params.gamma)
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
fn synthetic_output(
    vt: f64,
    params: &OutputParams,
    channel_conductance: f64,
    vg: f64,
    vds: &[f64],
) -> Vec<f64> {
    let overdrive = vg - vt;
    if overdrive <= 0.0 {
        return vec![0.0; vds.len()];
    }
    let saturation_voltage = params.alpha_sat * overdrive;
    vds.iter()
        .map(|&vd| {
            if vd <= 0.0 {
                return 0.0;
            }
            let effective_vd =
                vd / (1.0 + (vd / saturation_voltage).powf(params.m)).powf(1.0 / params.m);
            channel_conductance * effective_vd * (1.0 + params.lambda * vd)
        })
        .collect()
}

#[cfg(test)]
impl ModelFitState {
    /// Minimal library-unit fixture. Rich visual/reference fixtures live in the
    /// integration-test support module and are deliberately not duplicated here.
    pub(crate) fn load_demo(&mut self) {
        self.clear();
        let params = ModelParams {
            vt: 2.0,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg: Vec<_> = (0..=120).map(|idx| -2.0 + idx as f64 * 0.1).collect();
        for name in ["demo: organic", "demo: LTPS"] {
            let id = synthetic_transfer(&params, &vg);
            let mut device =
                FittedDevice::fit(name.to_owned(), vg.clone(), id).expect("test device fits");
            let output_source = (name == "demo: organic").then(|| {
                let output = OutputParams {
                    alpha_sat: 0.7,
                    lambda: 0.01,
                    m: 2.5,
                };
                let vds: Vec<_> = (0..=100).map(|idx| idx as f64 * 0.1).collect();
                let curves = [2.0_f64, 4.0, 6.0, 8.0]
                    .into_iter()
                    .map(|overdrive| OutputCurve {
                        vg: params.vt + overdrive,
                        id: synthetic_output(
                            params.vt,
                            &output,
                            params.k * overdrive.powf(1.0 + params.gamma),
                            params.vt + overdrive,
                            &vds,
                        ),
                        vds: vds.clone(),
                    })
                    .collect();
                assert!(device
                    .replace_output(curves)
                    .expect("device without retained DIBL accepts output")
                    .displaced
                    .is_empty());
                OutputSource::new("demo: organic_output.xlsx", None)
                    .expect("test source is visibly named")
            });
            let primary_source = PrimaryTransferSource::new(name, None)
                .expect("test primary source is visibly named");
            assert_eq!(
                self.install_fitted_device(device, primary_source, output_source)
                    .expect("test device and source agree"),
                DeviceInstallOutcome::Installed
            );
        }
    }
}

#[cfg(test)]
mod tests;
