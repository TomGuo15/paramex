//! Second-transfer parse and DIBL-refinement effects.

use std::path::PathBuf;

use eframe::egui;
use egui_notify::Toasts;
use paramex_core::modelfit::SecondTransfer;

use super::super::{start_dibl_refinement, Msg};
use super::visible_source_name;
use crate::format_ui::{
    model_fit_dibl_refinement_failure, MODEL_DIBL_AMBIGUOUS_MESSAGE, MODEL_DIBL_NO_MATCH_MESSAGE,
    MODEL_DIBL_SOURCE_NAME_MESSAGE,
};
use crate::io_tasks::{IoQueue, WorkerFailure};
use crate::workspaces::modelfit::state::{
    DeviceId, DiblCommitReport, DiblImport, DiblIssue, DiblIssueKind, DiblRefinementMode,
    DiblRefinementPurpose, DiblRefinementRecovery, DiblRefinementResult, DiblSource, IngestIssues,
    ModelFitState,
};

pub(in crate::workspaces::modelfit::ingest) fn apply_second_transfers_parsed(
    ctx: &egui::Context,
    single_target: Option<DeviceId>,
    outcomes: Vec<(PathBuf, String, Result<SecondTransfer, String>)>,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
) {
    let single_file_dialog = outcomes.len() == 1;
    let mut imports = Vec::new();
    let mut issues = Vec::new();
    for (source_path, name, result) in outcomes {
        match result {
            Ok(second) => match DiblSource::new(name.clone(), Some(source_path.clone())) {
                Ok(source) => imports.push(DiblImport { source, second }),
                Err(_) => {
                    let visible_name = visible_source_name(&name, &source_path);
                    tracing::warn!(
                        "Model Fit DIBL source has no visible file name: {}",
                        source_path.display()
                    );
                    issues.push(DiblIssue {
                        name: visible_name,
                        message: MODEL_DIBL_SOURCE_NAME_MESSAGE.to_owned(),
                        kind: DiblIssueKind::Parse,
                    });
                }
            },
            Err(message) => {
                tracing::warn!("Model Fit second-transfer parse failed for {name}: {message}");
                issues.push(DiblIssue {
                    name: visible_source_name(&name, &source_path),
                    message,
                    kind: DiblIssueKind::Parse,
                });
            }
        }
    }
    let plan = state.plan_dibl_refinement(imports, single_target, single_file_dialog, issues);
    start_dibl_refinement(ctx, io, plan);
}

pub(in crate::workspaces::modelfit::ingest) fn apply_dibl_refined(
    result: DiblRefinementResult,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    let report = state.commit_dibl_refinement(result);
    let toast = dibl_completion_toast(&report);
    for issue in report.issues {
        if matches!(issue.kind, DiblIssueKind::Parse | DiblIssueKind::NoFit) {
            issues.record(issue.name, issue.message);
        }
    }
    match toast {
        DiblCompletionToast::Info(message) => toasts.info(message),
        DiblCompletionToast::Success(message) => toasts.success(message),
        DiblCompletionToast::Warning(message) => toasts.warning(message),
        DiblCompletionToast::Error(message) => toasts.error(message),
    };
}

pub(in crate::workspaces::modelfit::ingest) fn apply_dibl_refinement_failed(
    recovery: DiblRefinementRecovery,
    failure: WorkerFailure,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    let report = state.recover_dibl_refinement(recovery);
    for issue in report.issues {
        if matches!(issue.kind, DiblIssueKind::Parse | DiblIssueKind::NoFit) {
            issues.record(issue.name, issue.message);
        }
    }
    let action = match report.purpose {
        DiblRefinementPurpose::Import => match report.mode {
            DiblRefinementMode::Single => "DIBL refinement",
            DiblRefinementMode::Batch => "DIBL batch refinement",
        },
        DiblRefinementPurpose::AttachPending => "Pending-DIBL attachment",
        DiblRefinementPurpose::Detach => "DIBL detachment",
        DiblRefinementPurpose::Remove => "DIBL removal",
    };
    let message = model_fit_dibl_refinement_failure(action, report.recovered);
    issues.record(failure.task_name().to_owned(), message.clone());
    toasts.error(message);
}

#[derive(Debug, PartialEq, Eq)]
enum DiblCompletionToast {
    Info(String),
    Success(String),
    Warning(String),
    Error(String),
}

fn dibl_completion_toast(report: &DiblCommitReport) -> DiblCompletionToast {
    match report.purpose {
        DiblRefinementPurpose::AttachPending => {
            return if report.action_succeeded {
                let suffix = if report.displaced > 0 {
                    " Previous DIBL measurement moved to pending."
                } else {
                    ""
                };
                DiblCompletionToast::Success(format!("Attached pending DIBL.{suffix}"))
            } else {
                DiblCompletionToast::Warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.clone())
                        .unwrap_or_else(|| "Could not attach the pending DIBL.".to_owned()),
                )
            };
        }
        DiblRefinementPurpose::Detach => {
            return if report.action_succeeded {
                DiblCompletionToast::Info("DIBL measurement moved to pending.".to_owned())
            } else {
                DiblCompletionToast::Warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.clone())
                        .unwrap_or_else(|| {
                            "Could not detach the attached DIBL measurement.".to_owned()
                        }),
                )
            };
        }
        DiblRefinementPurpose::Remove => {
            return if report.action_succeeded {
                DiblCompletionToast::Info("Removed attached DIBL measurement.".to_owned())
            } else {
                DiblCompletionToast::Warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.clone())
                        .unwrap_or_else(|| {
                            "Could not remove the attached DIBL measurement.".to_owned()
                        }),
                )
            };
        }
        DiblRefinementPurpose::Import => {}
    }
    if report.mode == DiblRefinementMode::Single {
        if let Some(fit) = report.fitted.first() {
            let suffix = if report.displaced > 0 {
                ". Previous DIBL measurement moved to pending."
            } else {
                ""
            };
            return DiblCompletionToast::Success(format!(
                "DIBL fitted from {} (V_DS {:.3} V): AT = {:.3e} m/V{suffix}",
                fit.name, fit.second_vds, fit.at,
            ));
        }
        if let Some(issue) = report.issues.first() {
            return if issue.kind == DiblIssueKind::Parse {
                DiblCompletionToast::Error(issue.message.clone())
            } else {
                DiblCompletionToast::Warning(format!(
                    "Loaded {} but no DIBL fit: {} Kept as pending.",
                    issue.name, issue.message
                ))
            };
        }
        if report.unmatched > 0 {
            return DiblCompletionToast::Warning(format!(
                "{MODEL_DIBL_NO_MATCH_MESSAGE} Kept as pending."
            ));
        }
        if report.ambiguous > 0 {
            return DiblCompletionToast::Warning(format!(
                "{MODEL_DIBL_AMBIGUOUS_MESSAGE} Kept as pending."
            ));
        }
        return DiblCompletionToast::Warning("No DIBL result was produced.".to_owned());
    }

    let (mut nofit, mut parse_errors) = (0usize, 0usize);
    for issue in &report.issues {
        match issue.kind {
            DiblIssueKind::Parse => parse_errors += 1,
            DiblIssueKind::NoFit => nofit += 1,
            DiblIssueKind::Stale | DiblIssueKind::Commit => {}
        }
    }
    let mut summary = format!(
        "DIBL batch: {} fitted, {} displaced, {nofit} no fit, {} unmatched, \
         {} ambiguous, {parse_errors} parse error(s), {} stale, \
         {} commit error(s).",
        report.fitted.len(),
        report.displaced,
        report.unmatched,
        report.ambiguous,
        report.stale,
        report.commit_errors,
    );
    let completed_without_warning = !report.fitted.is_empty()
        && nofit == 0
        && parse_errors == 0
        && report.unmatched == 0
        && report.ambiguous == 0
        && report.stale == 0
        && report.commit_errors == 0;
    if !completed_without_warning {
        if let Some(issue) = report.issues.first() {
            summary.push_str(" First issue: ");
            summary.push_str(&issue.message);
        }
        DiblCompletionToast::Warning(summary)
    } else {
        DiblCompletionToast::Success(summary)
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use paramex_core::modelfit::{FittedDevice, ModelParams};

    use super::*;
    use crate::io_tasks::IoEvent;
    use crate::workspaces::modelfit::state::{
        synthetic_transfer, DeviceInstallOutcome, DiblFit, IssueRow, PendingDiblReason,
        PrimaryTransferSource,
    };

    #[derive(Default)]
    struct TestIngest {
        io: IoQueue<Msg>,
        issues: IngestIssues,
    }

    fn finish_dibl_refinement(
        single_target: Option<DeviceId>,
        outcomes: Vec<(String, Result<SecondTransfer, String>)>,
        state: &mut ModelFitState,
        ingest: &mut TestIngest,
        toasts: &mut Toasts,
    ) {
        let ctx = egui::Context::default();
        let outcomes = outcomes
            .into_iter()
            .map(|(name, result)| (PathBuf::from(&name), name, result))
            .collect();
        apply_second_transfers_parsed(&ctx, single_target, outcomes, state, &mut ingest.io);
        assert!(ingest.io.is_busy());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = ingest.io.drain_events().into_iter().find_map(|event| {
                if let IoEvent::Message(Msg::DiblRefined { result }) = event {
                    Some(result)
                } else {
                    None
                }
            }) {
                apply_dibl_refined(result, state, &mut ingest.issues, toasts);
                return;
            }
            assert!(Instant::now() < deadline, "DIBL refinement timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn single_dibl_completion_preserves_detail_and_failure_severity() {
        let success = dibl_completion_toast(&DiblCommitReport {
            purpose: DiblRefinementPurpose::Import,
            mode: DiblRefinementMode::Single,
            fitted: vec![DiblFit {
                name: "low.csv".into(),
                second_vds: 2.0,
                at: 1.25e-8,
            }],
            displaced: 0,
            unmatched: 0,
            ambiguous: 0,
            stale: 0,
            commit_errors: 0,
            issues: Vec::new(),
            action_succeeded: true,
        });
        assert_eq!(
            success,
            DiblCompletionToast::Success(
                "DIBL fitted from low.csv (V_DS 2.000 V): AT = 1.250e-8 m/V".into()
            )
        );

        let parse = dibl_completion_toast(&DiblCommitReport {
            purpose: DiblRefinementPurpose::Import,
            mode: DiblRefinementMode::Single,
            fitted: Vec::new(),
            displaced: 0,
            unmatched: 0,
            ambiguous: 0,
            stale: 0,
            commit_errors: 0,
            issues: vec![DiblIssue {
                name: "bad.csv".into(),
                message: "missing Vg column".into(),
                kind: DiblIssueKind::Parse,
            }],
            action_succeeded: false,
        });
        assert_eq!(
            parse,
            DiblCompletionToast::Error("missing Vg column".into())
        );

        let stale = dibl_completion_toast(&DiblCommitReport {
            purpose: DiblRefinementPurpose::Import,
            mode: DiblRefinementMode::Single,
            fitted: Vec::new(),
            displaced: 0,
            unmatched: 0,
            ambiguous: 0,
            stale: 1,
            commit_errors: 0,
            issues: vec![DiblIssue {
                name: "low.csv".into(),
                message: "Device changed while DIBL refinement was running.".into(),
                kind: DiblIssueKind::Stale,
            }],
            action_succeeded: false,
        });
        assert_eq!(
            stale,
            DiblCompletionToast::Warning(
                "Loaded low.csv but no DIBL fit: Device changed while DIBL refinement was running. Kept as pending."
                    .into(),
            )
        );

        let removed = dibl_completion_toast(&DiblCommitReport {
            purpose: DiblRefinementPurpose::Remove,
            mode: DiblRefinementMode::Single,
            fitted: Vec::new(),
            displaced: 0,
            unmatched: 0,
            ambiguous: 0,
            stale: 0,
            commit_errors: 0,
            issues: Vec::new(),
            action_succeeded: true,
        });
        assert_eq!(
            removed,
            DiblCompletionToast::Info("Removed attached DIBL measurement.".into())
        );
    }

    #[test]
    fn mixed_dibl_batch_with_pending_input_is_a_warning() {
        let toast = dibl_completion_toast(&DiblCommitReport {
            purpose: DiblRefinementPurpose::Import,
            mode: DiblRefinementMode::Batch,
            fitted: vec![DiblFit {
                name: "fitted-low.csv".into(),
                second_vds: 2.0,
                at: 1.25e-8,
            }],
            displaced: 0,
            unmatched: 1,
            ambiguous: 0,
            stale: 0,
            commit_errors: 0,
            issues: vec![DiblIssue {
                name: "no-fit-low.csv".into(),
                message: "the pair did not improve the fit".into(),
                kind: DiblIssueKind::NoFit,
            }],
            action_succeeded: true,
        });

        assert!(matches!(toast, DiblCompletionToast::Warning(message)
            if message.contains("1 fitted")
                && message.contains("1 no fit")
                && message.contains("1 unmatched")
                && message.contains("First issue: the pair did not improve the fit")));
    }

    #[test]
    fn dibl_parse_failures_persist_their_file_names_in_the_model_file_list() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_dibl_refinement(
            None,
            vec![
                ("bad_dibl_a.csv".into(), Err("missing Vg column".into())),
                ("bad_dibl_b.csv".into(), Err("missing Id column".into())),
            ],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        let rows = ingest.issues.rows().collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        let names = rows.iter().map(|row| row.name).collect::<Vec<_>>();
        assert_eq!(names, ["bad_dibl_a.csv", "bad_dibl_b.csv"]);
    }

    #[test]
    fn unnamed_parsed_dibl_becomes_a_persistent_issue_before_refinement() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_dibl_refinement(
            None,
            vec![(
                String::new(),
                Ok(SecondTransfer {
                    vg: vec![0.0, 1.0],
                    id_abs: vec![1.0e-12, 2.0e-12],
                    v_ds: 1.0,
                }),
            )],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        let rows = ingest.issues.rows().collect::<Vec<_>>();
        assert!(matches!(
            rows.as_slice(),
            [IssueRow {
                name: "(unnamed file)",
                message: MODEL_DIBL_SOURCE_NAME_MESSAGE,
                ..
            }]
        ));
    }

    #[test]
    fn valid_dibl_batch_failures_stay_pending_without_parse_style_error_rows() {
        let mut state = ModelFitState::default();
        state.load_demo();
        let organic_vds = state
            .devices()
            .iter()
            .find(|entry| entry.device().name() == "demo: organic")
            .expect("organic demo row")
            .device()
            .bias()
            .v_ds;

        let params = ModelParams {
            vt: 2.0,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vgs = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
        let id = synthetic_transfer(&params, &vgs);
        for (path, device) in [
            (
                "lot-a/ambiguous_dibl.csv",
                FittedDevice::fit("ambiguous_dibl.csv".into(), vgs.clone(), id.clone())
                    .expect("first ambiguous test device fits"),
            ),
            (
                "lot-b/ambiguous_dibl.csv",
                FittedDevice::fit("ambiguous_dibl.csv".into(), vgs, id)
                    .expect("second ambiguous test device fits"),
            ),
        ] {
            assert_eq!(
                state
                    .install_fitted_device(
                        device,
                        PrimaryTransferSource::new("ambiguous_dibl.csv", Some(path.into()))
                            .unwrap(),
                        None,
                    )
                    .expect("test transfer has no output curves"),
                DeviceInstallOutcome::Installed
            );
        }

        let second = |v_ds| SecondTransfer {
            vg: vec![0.0, 1.0],
            id_abs: vec![1.0e-12, 2.0e-12],
            v_ds,
        };
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();
        finish_dibl_refinement(
            state.selected_device_id(),
            vec![
                ("demo: organic".into(), Ok(second(organic_vds))),
                ("missing_dibl.csv".into(), Ok(second(1.0))),
                ("ambiguous_dibl.csv".into(), Ok(second(1.0))),
            ],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        let rows = ingest.issues.rows().collect::<Vec<_>>();
        assert!(matches!(
            rows.as_slice(),
            [IssueRow {
                name: "demo: organic",
                ..
            }]
        ));
        let pending = state
            .pending_dibls()
            .iter()
            .map(|pending| (pending.name(), pending.reason()))
            .collect::<Vec<_>>();
        assert_eq!(pending.len(), 3);
        for expected in [
            ("demo: organic", PendingDiblReason::NoFit),
            ("missing_dibl.csv", PendingDiblReason::NoMatch),
            ("ambiguous_dibl.csv", PendingDiblReason::Ambiguous),
        ] {
            assert!(
                pending.contains(&expected),
                "missing pending row {expected:?}"
            );
        }
    }
}
