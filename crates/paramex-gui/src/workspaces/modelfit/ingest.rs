//! Model Fit transfer-file loading/fitting and Verilog-A export workers. Reuses
//! the Transfer parser, then completes the initial model fits before posting one
//! fit-ready batch to the UI thread.

use std::path::{Path, PathBuf};

use eframe::egui;
use paramex_core::modelfit::{
    extract_accumulation_capacitance_file, parse_output_file, parse_second_transfer_file,
    FittedDevice, OutputCurve, SecondTransfer, SUPPORTED_EXTENSIONS,
};
use paramex_core::transfer::parse_transfer_file;

use crate::io_tasks::{IoEvent, IoQueue, WorkerFailure};
use crate::workspaces::modelfit::state::{
    run_dibl_refinement, run_output_refinement, run_setup_refinement, DeviceId, DeviceToken,
    DiblRefinementPlan, DiblRefinementRecovery, DiblRefinementResult, OutputRefinementPlan,
    OutputRefinementRecovery, OutputRefinementResult, PrimaryTransferSource, SetupRefinementPlan,
    SetupRefinementResult,
};
use crate::workspaces::modelfit::ModelFitWorkspace;

mod effects;

/// A parser failure preserves its supplied message; a valid but unextractable
/// transfer uses the shared no-fit wording.
pub(crate) enum LoadError {
    Parse(String),
    Unfittable,
}

/// One terminal result from a Model Fit worker.
pub(crate) enum Msg {
    FilesFitted {
        outcomes: Vec<(PrimaryTransferSource, Result<FittedDevice, LoadError>)>,
    },
    OutputParsed {
        outcomes: Vec<(PathBuf, String, Result<Vec<OutputCurve>, String>)>,
    },
    OutputRefined {
        result: OutputRefinementResult,
    },
    OutputRefinementFailed {
        recovery: OutputRefinementRecovery,
        failure: WorkerFailure,
    },
    CvParsed {
        target: Option<DeviceToken>,
        name: String,
        result: Result<f64, String>,
    },
    CardExported {
        result: Result<PathBuf, String>,
    },
    SecondTransfersParsed {
        single_target: Option<DeviceId>,
        outcomes: Vec<(PathBuf, String, Result<SecondTransfer, String>)>,
    },
    DiblRefined {
        result: DiblRefinementResult,
    },
    DiblRefinementFailed {
        recovery: DiblRefinementRecovery,
        failure: WorkerFailure,
    },
    SetupRefined {
        result: Box<SetupRefinementResult>,
    },
}

pub(crate) fn drain(
    ctx: &egui::Context,
    workspace: &mut ModelFitWorkspace,
    toasts: &mut egui_notify::Toasts,
) {
    for event in workspace.io.drain_events() {
        match event {
            IoEvent::Message(Msg::FilesFitted { outcomes }) => effects::apply_files_fitted(
                outcomes,
                &mut workspace.state,
                &mut workspace.issues,
                toasts,
            ),
            IoEvent::Message(Msg::OutputParsed { outcomes }) => {
                effects::apply_output_parsed(ctx, outcomes, &mut workspace.state, &mut workspace.io)
            }
            IoEvent::Message(Msg::OutputRefined { result }) => effects::apply_output_refined(
                result,
                &mut workspace.state,
                &mut workspace.issues,
                toasts,
            ),
            IoEvent::Message(Msg::OutputRefinementFailed { recovery, failure }) => {
                effects::apply_output_refinement_failed(
                    recovery,
                    failure,
                    &mut workspace.state,
                    &mut workspace.issues,
                    toasts,
                )
            }
            IoEvent::Message(Msg::CvParsed {
                target,
                name,
                result,
            }) => effects::apply_cv_parsed(
                target,
                name,
                result,
                &mut workspace.state,
                &mut workspace.issues,
                toasts,
            ),
            IoEvent::Message(Msg::CardExported { result }) => {
                effects::apply_card_exported(result, toasts)
            }
            IoEvent::Message(Msg::SecondTransfersParsed {
                single_target,
                outcomes,
            }) => effects::apply_second_transfers_parsed(
                ctx,
                single_target,
                outcomes,
                &mut workspace.state,
                &mut workspace.io,
            ),
            IoEvent::Message(Msg::DiblRefined { result }) => effects::apply_dibl_refined(
                result,
                &mut workspace.state,
                &mut workspace.issues,
                toasts,
            ),
            IoEvent::Message(Msg::DiblRefinementFailed { recovery, failure }) => {
                effects::apply_dibl_refinement_failed(
                    recovery,
                    failure,
                    &mut workspace.state,
                    &mut workspace.issues,
                    toasts,
                )
            }
            IoEvent::Message(Msg::SetupRefined { result }) => {
                effects::apply_setup_refined(*result, &mut workspace.state, toasts)
            }
            IoEvent::WorkerFailed(failure) => {
                workspace
                    .issues
                    .record(failure.task_name().to_owned(), failure.message().to_owned());
                toasts.error(failure.notice());
            }
        }
    }
}

fn data_extensions() -> Vec<&'static str> {
    SUPPORTED_EXTENSIONS
        .iter()
        .map(|e| e.trim_start_matches('.'))
        .collect()
}

/// Parse and fit one transfer file completely on the worker path. An unfittable
/// curve is distinguished from a parse failure.
pub(super) fn fit_transfer_file(
    path: &Path,
) -> (PrimaryTransferSource, Result<FittedDevice, LoadError>) {
    let source = PrimaryTransferSource::from_path(path);
    let result = parse_transfer_file(path)
        .map_err(|e| LoadError::Parse(e.0))
        .and_then(|curve| {
            FittedDevice::fit(source.name().to_owned(), curve.vg, curve.id_abs)
                .map_err(|_| LoadError::Unfittable)
        });
    (source, result)
}

/// "Load Transfer": open the multi picker (blocking) on a worker thread,
/// parse and fit all picks in order, and send one batched [`Msg::FilesFitted`].
pub(crate) fn start_add_files(ctx: &egui::Context, queue: &mut IoQueue<Msg>) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit transfer load", || {
        let picked = rfd::FileDialog::new()
            .add_filter("data", &data_extensions())
            .set_title("Load transfer curves to fit")
            .pick_files();
        picked.map(|paths| {
            let outcomes = paths
                .into_iter()
                .map(|path| fit_transfer_file(&path))
                .collect();
            Msg::FilesFitted { outcomes }
        })
    });
}

/// "Load Output": open the multi picker on a worker thread, parse each
/// Id-Vd file, and send one batched [`Msg::OutputParsed`]. Each set is matched to a
/// device by base name when the message drains.
pub(crate) fn start_add_output_files(ctx: &egui::Context, queue: &mut IoQueue<Msg>) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit output load", || {
        let picked = rfd::FileDialog::new()
            .add_filter("data", &data_extensions())
            .set_title("Load Output (Id-Vd)")
            .pick_files();
        picked.map(|paths| {
            let outcomes = paths
                .into_iter()
                .map(|path| {
                    let name = crate::io_tasks::file_name_lossy(&path);
                    let result: Result<Vec<OutputCurve>, String> = parse_output_file(&path);
                    (path, name, result)
                })
                .collect();
            Msg::OutputParsed { outcomes }
        })
    });
}

/// "Load DIBL": open the picker on a worker thread, parse each sweep
/// together with its constant drain-bias column, and post one batched result. A
/// single pick attaches to the selected device; multiple picks pair by measurement
/// name.
pub(crate) fn start_add_second_transfer(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    single_target: Option<DeviceId>,
) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit DIBL load", move || {
        let picked = rfd::FileDialog::new()
            .add_filter("data", &data_extensions())
            .set_title("Load a 2nd transfer at a different V_DS (fit DIBL)")
            .pick_files();
        picked.map(|paths| {
            let outcomes = paths
                .into_iter()
                .map(|path| {
                    let name = crate::io_tasks::file_name_lossy(&path);
                    let result = parse_second_transfer_file(&path);
                    (path, name, result)
                })
                .collect();
            Msg::SecondTransfersParsed {
                single_target,
                outcomes,
            }
        })
    });
}

/// Stage two of output ingest/actions: refine owned device clones in one ordered
/// batch and post a guarded-commit result.
pub(crate) fn start_output_refinement(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    plan: OutputRefinementPlan,
) {
    start_output_refinement_with(ctx, queue, plan, run_output_refinement);
}

fn start_output_refinement_with<F>(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    plan: OutputRefinementPlan,
    refine: F,
) where
    F: FnOnce(OutputRefinementPlan) -> OutputRefinementResult + Send + 'static,
{
    let recovery = plan.panic_recovery();
    crate::io_tasks::spawn_io_with_panic_recovery(
        ctx,
        queue,
        "Model Fit output refinement",
        move || {
            Some(Msg::OutputRefined {
                result: refine(plan),
            })
        },
        move |failure| Msg::OutputRefinementFailed { recovery, failure },
    );
}

/// Stage two of DIBL ingest: refine owned device clones in one ordered batch and
/// post a guarded-commit result.
pub(crate) fn start_dibl_refinement(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    plan: DiblRefinementPlan,
) {
    start_dibl_refinement_with(ctx, queue, plan, run_dibl_refinement);
}

fn start_dibl_refinement_with<F>(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    plan: DiblRefinementPlan,
    refine: F,
) where
    F: FnOnce(DiblRefinementPlan) -> DiblRefinementResult + Send + 'static,
{
    let recovery = plan.panic_recovery();
    crate::io_tasks::spawn_io_with_panic_recovery(
        ctx,
        queue,
        "Model Fit DIBL refinement",
        move || {
            Some(Msg::DiblRefined {
                result: refine(plan),
            })
        },
        move |failure| Msg::DiblRefinementFailed { recovery, failure },
    );
}

pub(crate) fn start_setup_refinement(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    plan: SetupRefinementPlan,
) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit setup refinement", move || {
        Some(Msg::SetupRefined {
            result: Box::new(run_setup_refinement(plan)),
        })
    });
}

/// "Load C-V": open the single picker on a worker thread, parse the C-V sweep, and
/// extract the accumulation capacitance. Posts [`Msg::CvParsed`] with the source
/// name retained even when parsing fails; the UI thread applies `C_acc` only to
/// the exact device identity and scientific revision captured at click time.
pub(crate) fn start_add_cv_file(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    target: Option<DeviceToken>,
) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit C-V load", move || {
        let picked = rfd::FileDialog::new()
            .add_filter("data", &data_extensions())
            .set_title("Load a C-V sweep (extract Cox)")
            .pick_file();
        picked.map(|path| {
            let name = crate::io_tasks::file_name_lossy(&path);
            let result = extract_accumulation_capacitance_file(&path);
            Msg::CvParsed {
                target,
                name,
                result,
            }
        })
    });
}

/// "Export Verilog-A": save the already-rendered model-card bytes to an rfd-chosen
/// path. Posts [`Msg::CardExported`] or no message when cancelled.
pub(crate) fn start_export_card(
    ctx: &egui::Context,
    queue: &mut IoQueue<Msg>,
    bytes: Vec<u8>,
    default_name: String,
) {
    crate::io_tasks::spawn_io(ctx, queue, "Model Fit Verilog-A export", move || {
        let picked = rfd::FileDialog::new()
            .add_filter("model", &["va"])
            .set_file_name(&default_name)
            .set_title("Export Verilog-A")
            .save_file();
        picked.map(|path| {
            let result = std::fs::write(&path, &bytes)
                .map(|_| path)
                .map_err(|e| e.to_string());
            Msg::CardExported { result }
        })
    });
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::{Duration, Instant};

    use paramex_core::modelfit::{FittedDevice, ModelParams, OutputCurve, SecondTransfer};

    use super::*;
    use crate::workspaces::modelfit::state::{
        synthetic_transfer, DeviceInstallOutcome, DiblImport, DiblIssue, DiblIssueKind, DiblSource,
        OutputImport, OutputIssue, OutputSource, PendingDiblReason, PendingOutputReason,
    };

    fn add_device(state: &mut crate::workspaces::modelfit::state::ModelFitState, name: &str) {
        let params = ModelParams {
            vt: 1.0,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
        let device = FittedDevice::fit(
            name.to_owned(),
            vg.clone(),
            synthetic_transfer(&params, &vg),
        )
        .expect("test device fits");
        assert_eq!(
            state
                .install_fitted_device(
                    device,
                    PrimaryTransferSource::new(name, None).unwrap(),
                    None,
                )
                .expect("test transfer has no output curves"),
            DeviceInstallOutcome::Installed
        );
    }

    fn output_import(name: &str, path: &str) -> OutputImport {
        OutputImport {
            source: OutputSource::new(name, Some(path.into())).unwrap(),
            curves: vec![OutputCurve {
                vg: 4.0,
                vds: vec![0.0, 1.0],
                id: vec![0.0, 1.0e-6],
            }],
        }
    }

    fn second_transfer() -> SecondTransfer {
        let params = ModelParams {
            vt: 1.5,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
        SecondTransfer {
            id_abs: synthetic_transfer(&params, &vg),
            vg,
            v_ds: 1.0,
        }
    }

    fn drain_until_idle(
        ctx: &egui::Context,
        workspace: &mut ModelFitWorkspace,
        toasts: &mut egui_notify::Toasts,
    ) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while workspace.is_busy() && Instant::now() < deadline {
            drain(ctx, workspace, toasts);
            thread::sleep(Duration::from_millis(5));
        }
        drain(ctx, workspace, toasts);
        assert!(workspace.is_idle());
    }

    #[test]
    fn transfer_fit_worker_preserves_the_selected_source_path() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("paramex-core")
            .join("tests")
            .join("fixtures")
            .join("modelfit")
            .join("2-6.xlsx");

        let (source, result) = fit_transfer_file(&path);
        let device = match result {
            Ok(device) => device,
            Err(_) => panic!("fixture transfer fits"),
        };

        assert_eq!(source.name(), "2-6.xlsx");
        assert_eq!(source.path(), Some(path.as_path()));
        assert_eq!(device.name(), source.name());
    }

    #[test]
    fn panicked_output_refinement_recovers_every_input_and_closes_the_busy_slot() {
        let mut workspace = ModelFitWorkspace::default();
        add_device(&mut workspace.state, "dev_transfer.csv");
        let revision = workspace.state.selected_entry().unwrap().revision();
        let plan = workspace.state.plan_output_imports_with_issues(
            vec![
                output_import("dev_output.csv", "lot-a/dev_output.csv"),
                output_import("dev_id-vd.csv", "lot-b/dev_id-vd.csv"),
            ],
            vec![OutputIssue {
                name: "bad-output.csv".into(),
                message: "missing Vd column".into(),
                persist: true,
            }],
        );
        let ctx = egui::Context::default();
        let mut toasts = egui_notify::Toasts::default();

        start_output_refinement_with(&ctx, &mut workspace.io, plan, |_| {
            panic!("forced output refinement panic")
        });
        assert!(workspace.is_busy());
        drain_until_idle(&ctx, &mut workspace, &mut toasts);

        let entry = workspace.state.selected_entry().unwrap();
        assert_eq!(entry.revision(), revision);
        assert_eq!(entry.output_name(), None);
        assert_eq!(
            workspace
                .state
                .pending_outputs()
                .iter()
                .map(|pending| (pending.name(), pending.reason()))
                .collect::<Vec<_>>(),
            vec![
                ("dev_output.csv", PendingOutputReason::WorkerFailed),
                ("dev_id-vd.csv", PendingOutputReason::WorkerFailed),
            ]
        );
        let rows = workspace.issues.rows().collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.name == "bad-output.csv"));
        assert!(rows.iter().any(|row| {
            row.name == "Model Fit output refinement"
                && row
                    .message
                    .contains("Kept 2 parsed output measurement(s) pending")
        }));
        assert_eq!(toasts.len(), 1);
    }

    #[test]
    fn panicked_dibl_refinement_recovers_every_input_and_closes_the_busy_slot() {
        let mut workspace = ModelFitWorkspace::default();
        add_device(&mut workspace.state, "Id-Vg-high [(1) ; primary].csv");
        let revision = workspace.state.selected_entry().unwrap().revision();
        let names = [
            "Id-Vg-low [(1) ; first].csv",
            "Id-Vg-VD2V [(1) ; second].csv",
        ];
        let plan = workspace.state.plan_dibl_refinement(
            names
                .iter()
                .map(|name| DiblImport {
                    source: DiblSource::new(*name, None).unwrap(),
                    second: second_transfer(),
                })
                .collect(),
            None,
            false,
            vec![DiblIssue {
                name: "bad-dibl.csv".into(),
                message: "missing Vg column".into(),
                kind: DiblIssueKind::Parse,
            }],
        );
        let ctx = egui::Context::default();
        let mut toasts = egui_notify::Toasts::default();

        start_dibl_refinement_with(&ctx, &mut workspace.io, plan, |_| {
            panic!("forced DIBL refinement panic")
        });
        assert!(workspace.is_busy());
        drain_until_idle(&ctx, &mut workspace, &mut toasts);

        let entry = workspace.state.selected_entry().unwrap();
        assert_eq!(entry.revision(), revision);
        assert_eq!(entry.dibl_name(), None);
        assert_eq!(
            workspace
                .state
                .pending_dibls()
                .iter()
                .map(|pending| (pending.name(), pending.reason()))
                .collect::<Vec<_>>(),
            vec![
                (names[0], PendingDiblReason::WorkerFailed),
                (names[1], PendingDiblReason::WorkerFailed),
            ]
        );
        let rows = workspace.issues.rows().collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|row| row.name == "bad-dibl.csv"));
        assert!(rows.iter().any(|row| {
            row.name == "Model Fit DIBL refinement"
                && row
                    .message
                    .contains("Kept 2 parsed DIBL measurement(s) pending")
        }));
        assert_eq!(toasts.len(), 1);
    }
}
