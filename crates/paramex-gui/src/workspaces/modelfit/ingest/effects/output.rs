//! Output-measurement parse and refinement effects.

use std::path::PathBuf;

use eframe::egui;
use egui_notify::Toasts;
use paramex_core::modelfit::OutputCurve;

use super::super::{start_output_refinement, Msg};
use super::visible_source_name;
use crate::format_ui::{
    model_fit_output_refinement_failure, ATTACHED_PENDING_OUTPUT_MESSAGE,
    MODEL_OUTPUT_SOURCE_NAME_MESSAGE, OUTPUT_MOVED_TO_PENDING_MESSAGE, REMOVED_OUTPUT_MESSAGE,
};
use crate::io_tasks::{IoQueue, WorkerFailure};
use crate::workspaces::modelfit::state::{
    IngestIssues, ModelFitState, OutputImport, OutputIssue, OutputRefinementPurpose,
    OutputRefinementRecovery, OutputRefinementResult, OutputSource,
};
use crate::workspaces::output_ingest::OutputIngestStats;

pub(in crate::workspaces::modelfit::ingest) fn apply_output_parsed(
    ctx: &egui::Context,
    outcomes: Vec<(PathBuf, String, Result<Vec<OutputCurve>, String>)>,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
) {
    let mut imports = Vec::new();
    let mut parse_issues = Vec::new();
    for (source_path, name, result) in outcomes {
        match result {
            Ok(curves) => match OutputSource::new(name.clone(), Some(source_path.clone())) {
                Ok(source) => imports.push(OutputImport { source, curves }),
                Err(_) => {
                    let visible_name = visible_source_name(&name, &source_path);
                    tracing::warn!(
                        "Model Fit output source has no visible file name: {}",
                        source_path.display()
                    );
                    parse_issues.push(OutputIssue {
                        name: visible_name,
                        message: MODEL_OUTPUT_SOURCE_NAME_MESSAGE.to_owned(),
                        persist: true,
                    });
                }
            },
            Err(message) => {
                tracing::warn!("Model Fit output parse failed for {name}: {message}");
                parse_issues.push(OutputIssue {
                    name: visible_source_name(&name, &source_path),
                    message,
                    persist: true,
                });
            }
        }
    }
    let plan = state.plan_output_imports_with_issues(imports, parse_issues);
    start_output_refinement(ctx, io, plan);
}

pub(in crate::workspaces::modelfit::ingest) fn apply_output_refined(
    result: OutputRefinementResult,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    let report = state.commit_output_refinement(result);
    match report.purpose {
        OutputRefinementPurpose::Import => {
            let mut stats = OutputIngestStats::default();
            stats.attached = report.attached;
            stats.unfittable = report.unfittable;
            stats.unmatched = report.unmatched;
            stats.ambiguous = report.ambiguous;
            stats.displaced = report.displaced;
            for issue in report.issues {
                stats.record_error(issue.message.clone());
                if issue.persist {
                    issues.record(issue.name, issue.message);
                }
            }
            let summary = stats.model_fit_summary();
            let completed_without_warning = stats.attached > 0
                && stats.unfittable == 0
                && stats.unmatched == 0
                && stats.ambiguous == 0
                && stats.errors == 0;
            if completed_without_warning {
                toasts.success(summary);
            } else {
                toasts.warning(summary);
            }
        }
        OutputRefinementPurpose::AttachPending => {
            if report.action_succeeded {
                if report.unfittable > 0 {
                    toasts.warning("Attached pending output, but no output fit was extracted.");
                } else {
                    toasts.success(ATTACHED_PENDING_OUTPUT_MESSAGE);
                }
            } else {
                toasts.warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.as_str())
                        .unwrap_or("Could not attach the pending output."),
                );
            }
        }
        OutputRefinementPurpose::Detach => {
            if report.action_succeeded {
                toasts.info(OUTPUT_MOVED_TO_PENDING_MESSAGE);
            } else {
                toasts.warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.as_str())
                        .unwrap_or("Could not detach the output."),
                );
            }
        }
        OutputRefinementPurpose::Remove => {
            if report.action_succeeded {
                toasts.info(REMOVED_OUTPUT_MESSAGE);
            } else {
                toasts.warning(
                    report
                        .issues
                        .first()
                        .map(|issue| issue.message.as_str())
                        .unwrap_or("Could not remove the output."),
                );
            }
        }
    }
}

pub(in crate::workspaces::modelfit::ingest) fn apply_output_refinement_failed(
    recovery: OutputRefinementRecovery,
    failure: WorkerFailure,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    let report = state.recover_output_refinement(recovery);
    for issue in report.parse_issues {
        if issue.persist {
            issues.record(issue.name, issue.message);
        }
    }
    let action = match report.purpose {
        OutputRefinementPurpose::Import => "Output refinement",
        OutputRefinementPurpose::AttachPending => "Pending-output attachment",
        OutputRefinementPurpose::Detach => "Output detachment",
        OutputRefinementPurpose::Remove => "Output removal",
    };
    let message = model_fit_output_refinement_failure(action, report.recovered);
    issues.record(failure.task_name().to_owned(), message.clone());
    toasts.error(message);
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::io_tasks::IoEvent;
    use crate::workspaces::modelfit::state::IssueRow;

    #[derive(Default)]
    struct TestIngest {
        io: IoQueue<Msg>,
        issues: IngestIssues,
    }

    fn finish_output_refinement(
        outcomes: Vec<(PathBuf, String, Result<Vec<OutputCurve>, String>)>,
        state: &mut ModelFitState,
        ingest: &mut TestIngest,
        toasts: &mut Toasts,
    ) {
        let ctx = egui::Context::default();
        apply_output_parsed(&ctx, outcomes, state, &mut ingest.io);
        assert!(ingest.io.is_busy());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(result) = ingest.io.drain_events().into_iter().find_map(|event| {
                if let IoEvent::Message(Msg::OutputRefined { result }) = event {
                    Some(result)
                } else {
                    None
                }
            }) {
                apply_output_refined(result, state, &mut ingest.issues, toasts);
                return;
            }
            assert!(Instant::now() < deadline, "output refinement timed out");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn output_summary_surfaces_the_first_error_only_when_nothing_attached() {
        let mut stats = OutputIngestStats::default();
        stats.record_error("bad output header".to_string());
        let total_fail = stats.model_fit_summary();
        assert!(
            total_fail.contains("First error: bad output header"),
            "{total_fail}"
        );

        let mut stats = OutputIngestStats::default();
        stats.attached = 1;
        stats.record_error("bad output header".to_string());
        let mixed = stats.model_fit_summary();
        assert!(!mixed.contains("First error"), "{mixed}");
    }

    #[test]
    fn output_summary_reports_ambiguous_matches() {
        let mut stats = OutputIngestStats::default();
        stats.ambiguous = 2;
        let summary = stats.model_fit_summary();
        assert!(summary.contains("2 ambiguous"), "{summary}");
    }

    #[test]
    fn accepted_no_fit_output_is_reported_as_attached_and_unfittable() {
        let mut stats = OutputIngestStats::default();
        stats.attached = 1;
        stats.unfittable = 1;
        let summary = stats.model_fit_summary();
        assert!(summary.contains("1 attached"), "{summary}");
        assert!(summary.contains("1 unfittable"), "{summary}");

        stats.unfittable = 0;
        assert!(stats.model_fit_summary().contains("0 unfittable"));
    }

    #[test]
    fn output_parse_failure_persists_in_the_model_file_list() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_output_refinement(
            vec![(
                PathBuf::from("lot-a/bad_output.xlsx"),
                "bad_output.xlsx".into(),
                Err("missing Vd column".into()),
            )],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        assert!(ingest.issues.has_errors());
    }

    #[test]
    fn unnamed_parsed_output_becomes_a_persistent_issue_before_refinement() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_output_refinement(
            vec![(PathBuf::new(), String::new(), Ok(Vec::new()))],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        let rows = ingest.issues.rows().collect::<Vec<_>>();
        assert!(matches!(
            rows.as_slice(),
            [IssueRow {
                name: "(unnamed file)",
                message: MODEL_OUTPUT_SOURCE_NAME_MESSAGE,
                ..
            }]
        ));
        assert!(state.pending_outputs().is_empty());
    }

    #[test]
    fn unmatched_output_is_recoverable_as_pending_data() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_output_refinement(
            vec![(
                PathBuf::from("lot-a/orphan_output.xlsx"),
                "orphan_output.xlsx".into(),
                Ok(Vec::new()),
            )],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        assert_eq!(state.pending_outputs().len(), 1);
        assert_eq!(state.pending_outputs()[0].name(), "orphan_output.xlsx");
        assert!(!ingest.issues.has_errors());
    }

    #[test]
    fn same_named_outputs_from_different_paths_remain_separate() {
        let mut state = ModelFitState::default();
        let mut ingest = TestIngest::default();
        let mut toasts = Toasts::default();

        finish_output_refinement(
            vec![
                (
                    PathBuf::from("lot-a/orphan_output.xlsx"),
                    "orphan_output.xlsx".into(),
                    Ok(Vec::new()),
                ),
                (
                    PathBuf::from("lot-b/orphan_output.xlsx"),
                    "orphan_output.xlsx".into(),
                    Ok(Vec::new()),
                ),
            ],
            &mut state,
            &mut ingest,
            &mut toasts,
        );

        assert_eq!(state.pending_outputs().len(), 2);
        assert_ne!(
            state.pending_outputs()[0].source_path(),
            state.pending_outputs()[1].source_path()
        );
    }
}
