//! Output-measurement planning, background refinement, and guarded commit.

#[cfg(test)]
use paramex_core::modelfit::FittedDevice;
use paramex_core::modelfit::{DetachOutputError, OutputAttachOutcome, OutputCurve};

use super::super::pairing::device_base_name;
use super::super::{
    same_output_source, DeviceEntry, DeviceId, DeviceScience, DeviceToken, ModelFitState,
    OutputSource, PendingOutput, PendingOutputReason,
};
use super::pending::{apply_pending_effects, PendingEffect};
use crate::format_ui::{
    MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE, MODEL_OUTPUT_DIBL_CONFLICT_MESSAGE,
    MODEL_OUTPUT_STALE_MESSAGE,
};

#[derive(Clone)]
pub(crate) struct OutputImport {
    pub(crate) source: OutputSource,
    pub(crate) curves: Vec<OutputCurve>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputRefinementPurpose {
    Import,
    AttachPending,
    Detach,
    Remove,
}

#[derive(Clone)]
pub(crate) struct OutputIssue {
    pub(crate) name: String,
    pub(crate) message: String,
    pub(crate) persist: bool,
}

#[derive(Default)]
struct OutputImportSummary {
    unmatched: usize,
    ambiguous: usize,
    pending: Vec<DeferredOutput>,
    parse_issues: Vec<OutputIssue>,
}

#[derive(Clone)]
struct DeferredOutput {
    ordinal: usize,
    import: OutputImport,
    reason: PendingOutputReason,
}

enum OutputOperation {
    Replace {
        ordinal: usize,
        import: OutputImport,
    },
    Clear {
        keep_pending: bool,
    },
}

struct OutputJob {
    token: DeviceToken,
    device_name: String,
    science: DeviceScience,
    operations: Vec<OutputOperation>,
}

pub(crate) struct OutputRefinementPlan {
    purpose: OutputRefinementPurpose,
    summary: OutputImportSummary,
    jobs: Vec<OutputJob>,
}

pub(crate) struct OutputRefinementRecovery {
    purpose: OutputRefinementPurpose,
    pending: Vec<DeferredOutput>,
    parse_issues: Vec<OutputIssue>,
}

pub(crate) struct OutputRecoveryReport {
    pub(crate) purpose: OutputRefinementPurpose,
    pub(crate) recovered: usize,
    pub(crate) parse_issues: Vec<OutputIssue>,
}

struct RankedPendingOutput {
    generation: Option<usize>,
    pending: PendingOutput,
}

enum OutputOperationResult {
    Applied {
        ordinal: usize,
        import: OutputImport,
        outcome: OutputAttachOutcome,
        displaced: Option<RankedPendingOutput>,
    },
    DiblConflict {
        ordinal: usize,
        import: OutputImport,
    },
}

impl OutputOperationResult {
    fn into_import(self) -> (usize, OutputImport) {
        match self {
            Self::Applied {
                ordinal, import, ..
            }
            | Self::DiblConflict { ordinal, import } => (ordinal, import),
        }
    }
}

type OutputPendingEffect = PendingEffect<OutputSource, PendingOutput>;

struct OutputJobResult {
    token: DeviceToken,
    device_name: String,
    science: DeviceScience,
    operations: Vec<OutputOperationResult>,
    clear_pending: Option<PendingOutput>,
    changed: bool,
    clear_error: Option<String>,
}

fn apply_output_pending_effects(state: &mut ModelFitState, effects: Vec<OutputPendingEffect>) {
    apply_pending_effects(
        &mut state.pending_outputs,
        effects,
        |pending| &pending.source,
        same_output_source,
    );
}

pub(crate) struct OutputRefinementResult {
    purpose: OutputRefinementPurpose,
    summary: OutputImportSummary,
    jobs: Vec<OutputJobResult>,
}

pub(crate) struct OutputCommitReport {
    pub(crate) purpose: OutputRefinementPurpose,
    pub(crate) attached: usize,
    pub(crate) unfittable: usize,
    pub(crate) unmatched: usize,
    pub(crate) ambiguous: usize,
    pub(crate) displaced: usize,
    pub(crate) issues: Vec<OutputIssue>,
    pub(crate) action_succeeded: bool,
}

impl ModelFitState {
    #[cfg(test)]
    pub(crate) fn plan_output_imports(&self, imports: Vec<OutputImport>) -> OutputRefinementPlan {
        self.plan_output_imports_with_issues(imports, Vec::new())
    }

    pub(crate) fn plan_output_imports_with_issues(
        &self,
        imports: Vec<OutputImport>,
        parse_issues: Vec<OutputIssue>,
    ) -> OutputRefinementPlan {
        let mut plan = OutputRefinementPlan {
            purpose: OutputRefinementPurpose::Import,
            summary: OutputImportSummary {
                parse_issues,
                ..OutputImportSummary::default()
            },
            jobs: Vec::new(),
        };

        for (ordinal, import) in imports.into_iter().enumerate() {
            let base = device_base_name(import.source.name());
            let matches = self
                .devices
                .iter()
                .filter(|entry| device_base_name(entry.device().name()) == base)
                .map(|entry| entry.id)
                .take(2)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [id] => {
                    push_output_import(&mut plan.jobs, &self.devices, *id, ordinal, import);
                }
                [] => {
                    plan.summary.unmatched += 1;
                    plan.summary.pending.push(DeferredOutput {
                        ordinal,
                        import,
                        reason: PendingOutputReason::NoMatch,
                    });
                }
                _ => {
                    plan.summary.ambiguous += 1;
                    plan.summary.pending.push(DeferredOutput {
                        ordinal,
                        import,
                        reason: PendingOutputReason::Ambiguous,
                    });
                }
            }
        }
        plan
    }

    pub(crate) fn plan_pending_output_attach(
        &self,
        pending_idx: usize,
    ) -> Option<OutputRefinementPlan> {
        let entry = self.selected_entry()?;
        let pending = self.pending_outputs.get(pending_idx)?;
        Some(OutputRefinementPlan {
            purpose: OutputRefinementPurpose::AttachPending,
            summary: OutputImportSummary::default(),
            jobs: vec![OutputJob {
                token: entry.token(),
                device_name: entry.device().name().to_owned(),
                science: entry.science().clone(),
                operations: vec![OutputOperation::Replace {
                    ordinal: 0,
                    import: OutputImport {
                        source: pending.source.clone(),
                        curves: pending.curves.clone(),
                    },
                }],
            }],
        })
    }

    pub(crate) fn plan_output_clear(
        &self,
        device_idx: usize,
        purpose: OutputRefinementPurpose,
    ) -> Option<OutputRefinementPlan> {
        debug_assert!(matches!(
            purpose,
            OutputRefinementPurpose::Detach | OutputRefinementPurpose::Remove
        ));
        let entry = self.devices.get(device_idx)?;
        if !entry.device().has_output_curves() {
            return None;
        }
        Some(OutputRefinementPlan {
            purpose,
            summary: OutputImportSummary::default(),
            jobs: vec![OutputJob {
                token: entry.token(),
                device_name: entry.device().name().to_owned(),
                science: entry.science().clone(),
                operations: vec![OutputOperation::Clear {
                    keep_pending: purpose == OutputRefinementPurpose::Detach,
                }],
            }],
        })
    }

    pub(crate) fn commit_output_refinement(
        &mut self,
        result: OutputRefinementResult,
    ) -> OutputCommitReport {
        let OutputRefinementResult {
            purpose,
            summary,
            jobs,
        } = result;
        let mut report = OutputCommitReport {
            purpose,
            attached: 0,
            unfittable: 0,
            unmatched: summary.unmatched,
            ambiguous: summary.ambiguous,
            displaced: 0,
            issues: summary.parse_issues,
            action_succeeded: false,
        };
        let mut pending_effects = summary
            .pending
            .into_iter()
            .map(|pending| OutputPendingEffect::FreshPending {
                ordinal: pending.ordinal,
                pending: PendingOutput {
                    source: pending.import.source,
                    curves: pending.import.curves,
                    reason: pending.reason,
                },
            })
            .collect::<Vec<_>>();

        for job in jobs {
            let current_idx = self
                .devices
                .iter()
                .position(|entry| entry.id == job.token.id);
            let token_is_current = current_idx
                .and_then(|idx| self.devices.get(idx))
                .is_some_and(|entry| entry.revision == job.token.revision);

            if !token_is_current {
                if job.operations.is_empty() {
                    report.issues.push(OutputIssue {
                        name: job.device_name,
                        message: MODEL_OUTPUT_STALE_MESSAGE.to_owned(),
                        persist: false,
                    });
                }
                for operation in job.operations {
                    let (ordinal, import) = operation.into_import();
                    report.issues.push(OutputIssue {
                        name: import.source.name.clone(),
                        message: MODEL_OUTPUT_STALE_MESSAGE.to_owned(),
                        persist: false,
                    });
                    pending_effects.push(OutputPendingEffect::FreshPending {
                        ordinal,
                        pending: PendingOutput {
                            source: import.source,
                            curves: import.curves,
                            reason: PendingOutputReason::DeviceChanged,
                        },
                    });
                }
                continue;
            }

            if let Some(message) = job.clear_error {
                report.issues.push(OutputIssue {
                    name: job.device_name,
                    message,
                    persist: false,
                });
                continue;
            }

            let idx = current_idx.expect("current token has a live row");
            if job.changed {
                self.devices[idx].commit_science(job.science);
                report.action_succeeded = true;
            }

            for operation in job.operations {
                match operation {
                    OutputOperationResult::Applied {
                        ordinal,
                        import,
                        outcome,
                        displaced,
                    } => {
                        report.attached += 1;
                        if outcome == OutputAttachOutcome::NoFit {
                            report.unfittable += 1;
                        }
                        pending_effects.push(OutputPendingEffect::FreshClear {
                            ordinal,
                            source: import.source,
                        });
                        if let Some(displaced) = displaced {
                            report.displaced += 1;
                            pending_effects.push(match displaced.generation {
                                Some(ordinal) => OutputPendingEffect::FreshPending {
                                    ordinal,
                                    pending: displaced.pending,
                                },
                                None => OutputPendingEffect::PreCommandDisplacement {
                                    ordinal,
                                    pending: displaced.pending,
                                },
                            });
                        }
                    }
                    OutputOperationResult::DiblConflict { ordinal, import } => {
                        report.issues.push(OutputIssue {
                            name: import.source.name.clone(),
                            message: MODEL_OUTPUT_DIBL_CONFLICT_MESSAGE.to_owned(),
                            persist: false,
                        });
                        pending_effects.push(OutputPendingEffect::FreshPending {
                            ordinal,
                            pending: PendingOutput {
                                source: import.source,
                                curves: import.curves,
                                reason: PendingOutputReason::DiblConflict,
                            },
                        });
                    }
                }
            }
            if let Some(pending) = job.clear_pending {
                pending_effects.push(OutputPendingEffect::CurrentDetach(pending));
            }
        }
        apply_output_pending_effects(self, pending_effects);
        report
    }

    pub(crate) fn recover_output_refinement(
        &mut self,
        recovery: OutputRefinementRecovery,
    ) -> OutputRecoveryReport {
        let recovered_sources = recovery
            .pending
            .iter()
            .map(|pending| pending.import.source.clone())
            .collect::<Vec<_>>();
        for pending in recovery.pending {
            self.add_pending_output(pending.import.source, pending.import.curves, pending.reason);
        }
        let recovered = self
            .pending_outputs
            .iter()
            .filter(|pending| {
                recovered_sources
                    .iter()
                    .any(|source| same_output_source(&pending.source, source))
            })
            .count();
        OutputRecoveryReport {
            purpose: recovery.purpose,
            recovered,
            parse_issues: recovery.parse_issues,
        }
    }
}

impl OutputRefinementPlan {
    pub(crate) fn panic_recovery(&self) -> OutputRefinementRecovery {
        let mut pending = self.summary.pending.clone();
        pending.extend(self.jobs.iter().flat_map(|job| {
            job.operations
                .iter()
                .filter_map(|operation| match operation {
                    OutputOperation::Replace { ordinal, import } => Some(DeferredOutput {
                        ordinal: *ordinal,
                        import: import.clone(),
                        reason: PendingOutputReason::WorkerFailed,
                    }),
                    OutputOperation::Clear { .. } => None,
                })
        }));
        pending.sort_by_key(|pending| pending.ordinal);
        OutputRefinementRecovery {
            purpose: self.purpose,
            pending,
            parse_issues: self.summary.parse_issues.clone(),
        }
    }
}

fn push_output_import(
    jobs: &mut Vec<OutputJob>,
    devices: &[DeviceEntry],
    id: DeviceId,
    ordinal: usize,
    import: OutputImport,
) {
    if let Some(job) = jobs.iter_mut().find(|job| job.token.id == id) {
        job.operations
            .push(OutputOperation::Replace { ordinal, import });
        return;
    }
    let entry = devices
        .iter()
        .find(|entry| entry.id == id)
        .expect("paired row remains live while building one UI-thread plan");
    jobs.push(OutputJob {
        token: entry.token(),
        device_name: entry.device().name().to_owned(),
        science: entry.science().clone(),
        operations: vec![OutputOperation::Replace { ordinal, import }],
    });
}

pub(crate) fn run_output_refinement(plan: OutputRefinementPlan) -> OutputRefinementResult {
    let jobs = plan
        .jobs
        .into_iter()
        .map(|mut job| {
            let mut operations = Vec::new();
            let mut clear_pending = None;
            let mut changed = false;
            let mut clear_error = None;
            let mut current_generation = None;

            for operation in job.operations {
                match operation {
                    OutputOperation::Replace { ordinal, import } => {
                        match job
                            .science
                            .replacing_output(import.source.clone(), import.curves.clone())
                        {
                            Ok(replacement) => {
                                let displaced =
                                    replacement.displaced.map(|output| RankedPendingOutput {
                                        generation: current_generation,
                                        pending: PendingOutput {
                                            source: output.source,
                                            curves: output.curves,
                                            reason: PendingOutputReason::Detached,
                                        },
                                    });
                                job.science = replacement.science;
                                current_generation =
                                    job.science.output_source().is_some().then_some(ordinal);
                                operations.push(OutputOperationResult::Applied {
                                    ordinal,
                                    import,
                                    outcome: replacement.outcome,
                                    displaced,
                                });
                                changed = true;
                            }
                            Err(rejected) => {
                                operations.push(OutputOperationResult::DiblConflict {
                                    ordinal,
                                    import: OutputImport {
                                        source: rejected.source,
                                        curves: rejected.curves,
                                    },
                                });
                            }
                        }
                    }
                    OutputOperation::Clear { keep_pending } => {
                        let (science, detached) = match job.science.without_output() {
                            Ok(detached) => detached,
                            Err(error) => {
                                clear_error = Some(output_clear_error_message(error).to_owned());
                                break;
                            }
                        };
                        job.science = science;
                        if keep_pending {
                            clear_pending = Some(PendingOutput {
                                source: detached.source,
                                curves: detached.curves,
                                reason: PendingOutputReason::Detached,
                            });
                        }
                        changed = true;
                    }
                }
            }

            OutputJobResult {
                token: job.token,
                device_name: job.device_name,
                science: job.science,
                operations,
                clear_pending,
                changed,
                clear_error,
            }
        })
        .collect();
    OutputRefinementResult {
        purpose: plan.purpose,
        summary: plan.summary,
        jobs,
    }
}

fn output_clear_error_message(error: DetachOutputError) -> &'static str {
    match error {
        DetachOutputError::NoOutput => "No output is attached.",
        DetachOutputError::RetainedDiblNotApplied => MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE,
    }
}

#[cfg(test)]
mod tests;
