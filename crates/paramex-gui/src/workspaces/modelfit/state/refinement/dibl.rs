//! DIBL-measurement planning, background refinement, and guarded commit.

use paramex_core::modelfit::{DetachDiblError, SecondTransfer};

use super::super::pairing::dibl_pair_key;
use super::super::{
    dibl_error_message, same_dibl_source, DeviceEntry, DeviceId, DeviceScience, DeviceToken,
    DiblSource, ModelFitState, PendingDibl, PendingDiblReason,
};
use super::pending::{apply_pending_effects, PendingEffect};
use crate::format_ui::MODEL_DIBL_STALE_MESSAGE;

#[derive(Clone)]
pub(crate) struct DiblImport {
    pub(crate) source: DiblSource,
    pub(crate) second: SecondTransfer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiblIssueKind {
    Parse,
    NoFit,
    Stale,
    Commit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiblRefinementPurpose {
    Import,
    AttachPending,
    Detach,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiblRefinementMode {
    Single,
    Batch,
}

#[derive(Clone)]
pub(crate) struct DiblIssue {
    pub(crate) name: String,
    pub(crate) message: String,
    pub(crate) kind: DiblIssueKind,
}

struct DiblJob {
    token: DeviceToken,
    device_name: String,
    science: DeviceScience,
    operations: Vec<DiblOperation>,
}

#[derive(Default)]
struct DiblImportSummary {
    unmatched: usize,
    ambiguous: usize,
    stale: usize,
    pending: Vec<DeferredDibl>,
    issues: Vec<DiblIssue>,
}

#[derive(Clone)]
struct DeferredDibl {
    ordinal: usize,
    import: DiblImport,
    reason: PendingDiblReason,
}

pub(crate) struct DiblRefinementPlan {
    purpose: DiblRefinementPurpose,
    mode: DiblRefinementMode,
    jobs: Vec<DiblJob>,
    summary: DiblImportSummary,
}

pub(crate) struct DiblRefinementRecovery {
    purpose: DiblRefinementPurpose,
    mode: DiblRefinementMode,
    pending: Vec<DeferredDibl>,
    issues: Vec<DiblIssue>,
}

pub(crate) struct DiblRecoveryReport {
    pub(crate) purpose: DiblRefinementPurpose,
    pub(crate) mode: DiblRefinementMode,
    pub(crate) recovered: usize,
    pub(crate) issues: Vec<DiblIssue>,
}

struct RankedPendingDibl {
    generation: Option<usize>,
    pending: PendingDibl,
}

enum DiblOperationResult {
    Fitted {
        ordinal: usize,
        import: DiblImport,
        at: f64,
        displaced: Option<RankedPendingDibl>,
    },
    NoFit {
        ordinal: usize,
        import: DiblImport,
        message: String,
    },
}

impl DiblOperationResult {
    fn into_import(self) -> (usize, DiblImport) {
        match self {
            Self::Fitted {
                ordinal, import, ..
            }
            | Self::NoFit {
                ordinal, import, ..
            } => (ordinal, import),
        }
    }
}

enum DiblOperation {
    Replace { ordinal: usize, import: DiblImport },
    Clear { keep_pending: bool },
}

type DiblPendingEffect = PendingEffect<DiblSource, PendingDibl>;

struct DiblJobResult {
    token: DeviceToken,
    device_name: String,
    science: DeviceScience,
    operations: Vec<DiblOperationResult>,
    clear_pending: Option<PendingDibl>,
    changed: bool,
    clear_error: Option<String>,
}

pub(crate) struct DiblRefinementResult {
    purpose: DiblRefinementPurpose,
    mode: DiblRefinementMode,
    jobs: Vec<DiblJobResult>,
    summary: DiblImportSummary,
}

fn apply_dibl_pending_effects(state: &mut ModelFitState, effects: Vec<DiblPendingEffect>) {
    apply_pending_effects(
        &mut state.pending_dibls,
        effects,
        |pending| &pending.source,
        same_dibl_source,
    );
}

pub(crate) struct DiblFit {
    pub(crate) name: String,
    pub(crate) second_vds: f64,
    pub(crate) at: f64,
}

pub(crate) struct DiblCommitReport {
    pub(crate) purpose: DiblRefinementPurpose,
    pub(crate) mode: DiblRefinementMode,
    pub(crate) fitted: Vec<DiblFit>,
    pub(crate) displaced: usize,
    pub(crate) unmatched: usize,
    pub(crate) ambiguous: usize,
    pub(crate) stale: usize,
    pub(crate) commit_errors: usize,
    pub(crate) issues: Vec<DiblIssue>,
    pub(crate) action_succeeded: bool,
}

impl ModelFitState {
    pub(crate) fn plan_dibl_refinement(
        &self,
        imports: Vec<DiblImport>,
        single_target: Option<DeviceId>,
        single_file_dialog: bool,
        parse_issues: Vec<DiblIssue>,
    ) -> DiblRefinementPlan {
        let mut jobs = Vec::new();
        let mut summary = DiblImportSummary {
            issues: parse_issues,
            ..DiblImportSummary::default()
        };
        for (ordinal, import) in imports.into_iter().enumerate() {
            let matches = if single_file_dialog {
                single_target
                    .filter(|id| self.devices.iter().any(|entry| entry.id == *id))
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                let key = dibl_pair_key(import.source.name());
                self.devices
                    .iter()
                    .filter(|entry| dibl_pair_key(entry.device().name()) == key)
                    .map(|entry| entry.id)
                    .take(2)
                    .collect::<Vec<_>>()
            };
            match matches.as_slice() {
                [id] => push_dibl_import(&mut jobs, &self.devices, *id, ordinal, import),
                [] => {
                    let reason = if single_file_dialog {
                        summary.stale += 1;
                        PendingDiblReason::DeviceChanged
                    } else {
                        summary.unmatched += 1;
                        PendingDiblReason::NoMatch
                    };
                    let name = import.source.name.clone();
                    summary.pending.push(DeferredDibl {
                        ordinal,
                        import,
                        reason,
                    });
                    if single_file_dialog {
                        summary.issues.push(DiblIssue {
                            name,
                            message: MODEL_DIBL_STALE_MESSAGE.to_owned(),
                            kind: DiblIssueKind::Stale,
                        });
                    }
                }
                _ => {
                    summary.ambiguous += 1;
                    summary.pending.push(DeferredDibl {
                        ordinal,
                        import,
                        reason: PendingDiblReason::Ambiguous,
                    });
                }
            }
        }
        DiblRefinementPlan {
            purpose: DiblRefinementPurpose::Import,
            mode: if single_file_dialog {
                DiblRefinementMode::Single
            } else {
                DiblRefinementMode::Batch
            },
            jobs,
            summary,
        }
    }

    pub(crate) fn plan_pending_dibl_attach(
        &self,
        pending_idx: usize,
    ) -> Option<DiblRefinementPlan> {
        let entry = self.selected_entry()?;
        let pending = self.pending_dibls.get(pending_idx)?;
        Some(DiblRefinementPlan {
            purpose: DiblRefinementPurpose::AttachPending,
            mode: DiblRefinementMode::Single,
            jobs: vec![DiblJob {
                token: entry.token(),
                device_name: entry.device().name().to_owned(),
                science: entry.science().clone(),
                operations: vec![DiblOperation::Replace {
                    ordinal: 0,
                    import: DiblImport {
                        source: pending.source.clone(),
                        second: pending.second.clone(),
                    },
                }],
            }],
            summary: DiblImportSummary::default(),
        })
    }

    pub(crate) fn plan_dibl_clear(
        &self,
        device_idx: usize,
        purpose: DiblRefinementPurpose,
    ) -> Option<DiblRefinementPlan> {
        debug_assert!(matches!(
            purpose,
            DiblRefinementPurpose::Detach | DiblRefinementPurpose::Remove
        ));
        let entry = self.devices.get(device_idx)?;
        if !entry.device().has_second_transfer() {
            return None;
        }
        Some(DiblRefinementPlan {
            purpose,
            mode: DiblRefinementMode::Single,
            jobs: vec![DiblJob {
                token: entry.token(),
                device_name: entry.device().name().to_owned(),
                science: entry.science().clone(),
                operations: vec![DiblOperation::Clear {
                    keep_pending: purpose == DiblRefinementPurpose::Detach,
                }],
            }],
            summary: DiblImportSummary::default(),
        })
    }

    pub(crate) fn commit_dibl_refinement(
        &mut self,
        result: DiblRefinementResult,
    ) -> DiblCommitReport {
        let DiblRefinementResult {
            purpose,
            mode,
            jobs,
            summary,
        } = result;
        let mut report = DiblCommitReport {
            purpose,
            mode,
            fitted: Vec::new(),
            displaced: 0,
            unmatched: summary.unmatched,
            ambiguous: summary.ambiguous,
            stale: summary.stale,
            commit_errors: 0,
            issues: summary.issues,
            action_succeeded: false,
        };
        let mut pending_effects = summary
            .pending
            .into_iter()
            .map(|pending| DiblPendingEffect::FreshPending {
                ordinal: pending.ordinal,
                pending: PendingDibl {
                    source: pending.import.source,
                    second: pending.import.second,
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
                    report.stale += 1;
                    report.issues.push(DiblIssue {
                        name: job.device_name,
                        message: MODEL_DIBL_STALE_MESSAGE.to_owned(),
                        kind: DiblIssueKind::Stale,
                    });
                }
                for operation in job.operations {
                    let (ordinal, import) = operation.into_import();
                    report.stale += 1;
                    report.issues.push(DiblIssue {
                        name: import.source.name.clone(),
                        message: MODEL_DIBL_STALE_MESSAGE.to_owned(),
                        kind: DiblIssueKind::Stale,
                    });
                    pending_effects.push(DiblPendingEffect::FreshPending {
                        ordinal,
                        pending: PendingDibl {
                            source: import.source,
                            second: import.second,
                            reason: PendingDiblReason::DeviceChanged,
                        },
                    });
                }
                continue;
            }

            if let Some(message) = job.clear_error {
                report.commit_errors += 1;
                report.issues.push(DiblIssue {
                    name: job.device_name,
                    message,
                    kind: DiblIssueKind::Commit,
                });
                continue;
            }

            if job.changed {
                let idx = current_idx.expect("current token has a live row");
                self.devices[idx].commit_science(job.science);
                report.action_succeeded = true;
            }

            for operation in job.operations {
                match operation {
                    DiblOperationResult::Fitted {
                        ordinal,
                        import,
                        at,
                        displaced,
                    } => {
                        pending_effects.push(DiblPendingEffect::FreshClear {
                            ordinal,
                            source: import.source.clone(),
                        });
                        report.fitted.push(DiblFit {
                            name: import.source.name,
                            second_vds: import.second.v_ds,
                            at,
                        });
                        if let Some(pending) = displaced {
                            report.displaced += 1;
                            pending_effects.push(match pending.generation {
                                Some(ordinal) => DiblPendingEffect::FreshPending {
                                    ordinal,
                                    pending: pending.pending,
                                },
                                None => DiblPendingEffect::PreCommandDisplacement {
                                    ordinal,
                                    pending: pending.pending,
                                },
                            });
                        }
                    }
                    DiblOperationResult::NoFit {
                        ordinal,
                        import,
                        message,
                    } => {
                        let name = import.source.name.clone();
                        pending_effects.push(DiblPendingEffect::FreshPending {
                            ordinal,
                            pending: PendingDibl {
                                source: import.source,
                                second: import.second,
                                reason: PendingDiblReason::NoFit,
                            },
                        });
                        report.issues.push(DiblIssue {
                            name,
                            message,
                            kind: DiblIssueKind::NoFit,
                        });
                    }
                }
            }
            if let Some(pending) = job.clear_pending {
                pending_effects.push(DiblPendingEffect::CurrentDetach(pending));
            }
        }
        apply_dibl_pending_effects(self, pending_effects);
        report
    }

    pub(crate) fn recover_dibl_refinement(
        &mut self,
        recovery: DiblRefinementRecovery,
    ) -> DiblRecoveryReport {
        let recovered_sources = recovery
            .pending
            .iter()
            .map(|pending| pending.import.source.clone())
            .collect::<Vec<_>>();
        for pending in recovery.pending {
            self.add_pending_dibl(pending.import.source, pending.import.second, pending.reason);
        }
        let recovered = self
            .pending_dibls
            .iter()
            .filter(|pending| {
                recovered_sources
                    .iter()
                    .any(|source| same_dibl_source(&pending.source, source))
            })
            .count();
        DiblRecoveryReport {
            purpose: recovery.purpose,
            mode: recovery.mode,
            recovered,
            issues: recovery.issues,
        }
    }
}

impl DiblRefinementPlan {
    pub(crate) fn panic_recovery(&self) -> DiblRefinementRecovery {
        let mut pending = self.summary.pending.clone();
        pending.extend(self.jobs.iter().flat_map(|job| {
            job.operations
                .iter()
                .filter_map(|operation| match operation {
                    DiblOperation::Replace { ordinal, import } => Some(DeferredDibl {
                        ordinal: *ordinal,
                        import: import.clone(),
                        reason: PendingDiblReason::WorkerFailed,
                    }),
                    DiblOperation::Clear { .. } => None,
                })
        }));
        pending.sort_by_key(|pending| pending.ordinal);
        DiblRefinementRecovery {
            purpose: self.purpose,
            mode: self.mode,
            pending,
            issues: self.summary.issues.clone(),
        }
    }
}

fn push_dibl_import(
    jobs: &mut Vec<DiblJob>,
    devices: &[DeviceEntry],
    id: DeviceId,
    ordinal: usize,
    import: DiblImport,
) {
    if let Some(job) = jobs.iter_mut().find(|job| job.token.id == id) {
        job.operations
            .push(DiblOperation::Replace { ordinal, import });
        return;
    }
    let entry = devices
        .iter()
        .find(|entry| entry.id == id)
        .expect("paired row remains live while building one UI-thread plan");
    jobs.push(DiblJob {
        token: entry.token(),
        device_name: entry.device().name().to_owned(),
        science: entry.science().clone(),
        operations: vec![DiblOperation::Replace { ordinal, import }],
    });
}

pub(crate) fn run_dibl_refinement(plan: DiblRefinementPlan) -> DiblRefinementResult {
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
                    DiblOperation::Replace { ordinal, import } => {
                        let primary_vds = job.science.device().bias().v_ds;
                        let second_vds = import.second.v_ds;
                        match job
                            .science
                            .replacing_second_transfer(import.source.clone(), import.second.clone())
                        {
                            Ok(replacement) => {
                                let displaced =
                                    replacement.displaced.map(|dibl| RankedPendingDibl {
                                        generation: current_generation,
                                        pending: PendingDibl {
                                            source: dibl.source,
                                            second: dibl.second,
                                            reason: PendingDiblReason::Detached,
                                        },
                                    });
                                job.science = replacement.science;
                                current_generation = Some(ordinal);
                                changed = true;
                                operations.push(DiblOperationResult::Fitted {
                                    ordinal,
                                    import,
                                    at: replacement.at,
                                    displaced,
                                });
                            }
                            Err(rejected) => operations.push(DiblOperationResult::NoFit {
                                ordinal,
                                import: DiblImport {
                                    source: rejected.source,
                                    second: rejected.second,
                                },
                                message: dibl_error_message(
                                    rejected.reason,
                                    primary_vds,
                                    second_vds,
                                ),
                            }),
                        }
                    }
                    DiblOperation::Clear { keep_pending } => {
                        match job.science.without_second_transfer() {
                            Ok((science, detached)) => {
                                job.science = science;
                                if keep_pending {
                                    clear_pending = Some(PendingDibl {
                                        source: detached.source,
                                        second: detached.second,
                                        reason: PendingDiblReason::Detached,
                                    });
                                }
                                changed = true;
                            }
                            Err(error) => {
                                clear_error = Some(dibl_detach_error_message(error));
                            }
                        }
                    }
                }
            }
            DiblJobResult {
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
    DiblRefinementResult {
        purpose: plan.purpose,
        mode: plan.mode,
        jobs,
        summary: plan.summary,
    }
}

fn dibl_detach_error_message(error: DetachDiblError) -> String {
    match error {
        DetachDiblError::NoSecondTransfer => "No DIBL measurement is attached.".to_owned(),
        DetachDiblError::CannotRestoreLevel62Fit => {
            "Could not rebuild Level 62 without the attached DIBL measurement.".to_owned()
        }
    }
}

#[cfg(test)]
mod tests;
