//! Model Fit ingest-message effects: install worker-fitted transfer devices.
//! Unfittable curves and parse errors are counted and summarized
//! in a toast, logged, and shown in the Model Fit error-row surface. When a batch
//! loads NOTHING, the toast also carries the first parser message (matching the C-V
//! loader) so the user learns *why* without opening the log.

use std::path::PathBuf;

use egui_notify::Toasts;
use paramex_core::modelfit::FittedDevice;

use super::LoadError;
use crate::format_ui::{
    exported_to, model_fit_loaded_devices_summary, MODEL_CV_STALE_MESSAGE,
    MODEL_DEVICE_REQUIRED_MESSAGE, MODEL_DIBL_REAPPLY_FAILED_MESSAGE,
    MODEL_PARAMETER_INVALID_MESSAGE, MODEL_REFIT_FAILED_MESSAGE, MODEL_SETUP_STALE_MESSAGE,
    VDS_POSITIVE_MESSAGE, WL_MODEL_INCOMPATIBLE_MESSAGE, WL_POSITIVE_MESSAGE,
};
use crate::io_tasks::saved_file_name;
use crate::workspaces::modelfit::state::{
    CvCommitError, DeviceInstallOutcome, DeviceToken, IngestIssues, ModelFitState,
    PrimaryTransferSource, SelectedMutationError, SetupCommitOutcome, SetupRefinementError,
    SetupRefinementPurpose, SetupRefinementResult,
};

mod dibl;
mod output;

pub(super) use dibl::{
    apply_dibl_refined, apply_dibl_refinement_failed, apply_second_transfers_parsed,
};
pub(super) use output::{
    apply_output_parsed, apply_output_refined, apply_output_refinement_failed,
};

pub(super) fn apply_files_fitted(
    outcomes: Vec<(PrimaryTransferSource, Result<FittedDevice, LoadError>)>,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    let (mut fitted, mut already_loaded, mut unfittable, mut errors) =
        (0usize, 0usize, 0usize, 0usize);
    let mut first_err: Option<String> = None;
    for (source, result) in outcomes {
        let name = source.name().to_owned();
        match result {
            Ok(device) => match state
                .install_fitted_device(device, source, None)
                .expect("freshly fitted transfer device has matching primary provenance")
            {
                DeviceInstallOutcome::Installed => fitted += 1,
                DeviceInstallOutcome::AlreadyLoaded => already_loaded += 1,
            },
            Err(LoadError::Unfittable) => {
                unfittable += 1;
                let message = "No extractable above-threshold region.".to_string();
                tracing::warn!("Model Fit: {name} has no extractable above-threshold region");
                issues.record(name, message);
            }
            Err(LoadError::Parse(message)) => {
                errors += 1;
                tracing::warn!("Model Fit parse failed for {name}: {message}");
                first_err.get_or_insert_with(|| message.clone());
                issues.record(name, message);
            }
        }
    }
    let summary = model_fit_loaded_devices_summary(
        fitted,
        already_loaded,
        unfittable,
        errors,
        first_err.as_deref(),
    );
    if fitted > 0 {
        toasts.success(summary);
    } else {
        toasts.warning(summary);
    }
}

pub(super) fn apply_cv_parsed(
    target: Option<DeviceToken>,
    name: String,
    result: Result<f64, String>,
    state: &mut ModelFitState,
    issues: &mut IngestIssues,
    toasts: &mut Toasts,
) {
    match result {
        Ok(c_acc) => match state.commit_cox_from_cv(target, c_acc) {
            Ok(cox) => {
                toasts.success(format!(
                    "Cox set to {:.3e} F/m\u{00B2} from {name} (C_acc {c_acc:.3e} F)",
                    cox
                ));
            }
            Err(error) => {
                let message = match error {
                    CvCommitError::DeviceChanged => MODEL_CV_STALE_MESSAGE,
                    CvCommitError::Mutation(SelectedMutationError::NoDeviceSelected) => {
                        MODEL_DEVICE_REQUIRED_MESSAGE
                    }
                    _ => MODEL_PARAMETER_INVALID_MESSAGE,
                };
                tracing::warn!("Model Fit could not apply C-V data from {name}: {message}");
                issues.record(name.clone(), message.to_owned());
                toasts.warning(format!("Loaded {name} but could not set Cox: {message}"));
            }
        },
        Err(message) => {
            tracing::warn!("Model Fit C-V parse failed: {message}");
            issues.record(name, message.clone());
            toasts.error(message);
        }
    };
}

pub(super) fn apply_setup_refined(
    result: SetupRefinementResult,
    state: &mut ModelFitState,
    toasts: &mut Toasts,
) {
    let report = state.commit_setup_refinement(result);
    match report.outcome {
        SetupCommitOutcome::Applied => {}
        SetupCommitOutcome::Stale => {
            tracing::warn!(
                "Model Fit setup result for {} was stale",
                report.device_name
            );
            toasts.warning(MODEL_SETUP_STALE_MESSAGE);
        }
        SetupCommitOutcome::Rejected(error) => {
            let message = setup_refinement_error_message(report.purpose, error);
            tracing::warn!(
                "Model Fit setup refinement failed for {}: {error:?}",
                report.device_name
            );
            toasts.warning(message);
        }
    }
}

fn setup_refinement_error_message(
    purpose: SetupRefinementPurpose,
    error: SetupRefinementError,
) -> &'static str {
    match (purpose, error) {
        (
            SetupRefinementPurpose::Geometry | SetupRefinementPurpose::DrainBias,
            SetupRefinementError::Input(paramex_core::modelfit::InputError::RetainedDiblNotApplied),
        )
        | (
            SetupRefinementPurpose::Reset,
            SetupRefinementError::Refit(paramex_core::modelfit::RefitError::RetainedDiblNotApplied),
        ) => MODEL_DIBL_REAPPLY_FAILED_MESSAGE,
        (
            SetupRefinementPurpose::Geometry,
            SetupRefinementError::Input(paramex_core::modelfit::InputError::InvalidGeometry),
        ) => WL_MODEL_INCOMPATIBLE_MESSAGE,
        (SetupRefinementPurpose::Geometry, SetupRefinementError::Input(_)) => WL_POSITIVE_MESSAGE,
        (
            SetupRefinementPurpose::DrainBias,
            SetupRefinementError::Input(paramex_core::modelfit::InputError::InvalidBias),
        ) => VDS_POSITIVE_MESSAGE,
        (SetupRefinementPurpose::DrainBias, SetupRefinementError::Input(_)) => {
            MODEL_PARAMETER_INVALID_MESSAGE
        }
        (SetupRefinementPurpose::Reset, SetupRefinementError::Refit(_)) => {
            MODEL_REFIT_FAILED_MESSAGE
        }
        _ => MODEL_PARAMETER_INVALID_MESSAGE,
    }
}

pub(super) fn apply_card_exported(result: Result<PathBuf, String>, toasts: &mut Toasts) {
    match result {
        Ok(path) => {
            toasts.success(exported_to(&saved_file_name(&path)));
        }
        Err(message) => {
            tracing::error!("Model card export failed: {message}");
            toasts.error(message);
        }
    }
}

fn visible_source_name(name: &str, source_path: &std::path::Path) -> String {
    if !name.trim().is_empty() {
        return name.to_owned();
    }
    let path = source_path.display().to_string();
    if path.trim().is_empty() {
        "(unnamed file)".to_owned()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use paramex_core::modelfit::{GeometryParams, ModelParams};

    use super::*;
    use crate::workspaces::modelfit::state::{
        run_setup_refinement, synthetic_transfer, IssueRow, SetupOperation,
    };

    fn fitted_device(name: &str, vt: f64) -> FittedDevice {
        let params = ModelParams {
            vt,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg = (0..=120)
            .map(|idx| -2.0 + idx as f64 * 0.1)
            .collect::<Vec<_>>();
        let id = synthetic_transfer(&params, &vg);
        FittedDevice::fit(name.to_owned(), vg, id).expect("test device fits")
    }

    #[test]
    fn setup_reducer_commits_a_successful_worker_device() {
        let mut state = ModelFitState::default();
        state.load_demo();
        let revision = state.selected_entry().unwrap().revision();
        let result = run_setup_refinement(
            state
                .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
                    w_um: 42.0,
                    l_um: 7.0,
                }))
                .unwrap()
                .unwrap(),
        );

        apply_setup_refined(result, &mut state, &mut Toasts::default());

        let selected = state.selected_entry().unwrap();
        assert_eq!(selected.device().geometry().w_um, 42.0);
        assert_eq!(selected.revision().get(), revision.get() + 1);
    }

    #[test]
    fn setup_reducer_keeps_live_state_on_rejection_and_stale_result() {
        let mut rejected = ModelFitState::default();
        rejected.load_demo();
        let before = rejected.selected_entry().unwrap().device().clone();
        let result = run_setup_refinement(
            rejected
                .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
                    w_um: 0.0,
                    l_um: 7.0,
                }))
                .unwrap()
                .unwrap(),
        );
        apply_setup_refined(result, &mut rejected, &mut Toasts::default());
        assert_eq!(rejected.selected_entry().unwrap().device(), &before);

        let mut stale = ModelFitState::default();
        stale.load_demo();
        let result = run_setup_refinement(
            stale
                .plan_selected_setup(SetupOperation::DrainBias(0.25))
                .unwrap()
                .unwrap(),
        );
        assert!(stale.remove_device(0));
        let survivor = stale.selected_entry().unwrap().device().clone();
        apply_setup_refined(result, &mut stale, &mut Toasts::default());
        assert_eq!(stale.selected_entry().unwrap().device(), &survivor);
    }

    #[test]
    fn retained_dibl_setup_rejections_use_resolution_copy_for_every_boundary() {
        use paramex_core::modelfit::{InputError, RefitError};

        for (purpose, error) in [
            (
                SetupRefinementPurpose::Geometry,
                SetupRefinementError::Input(InputError::RetainedDiblNotApplied),
            ),
            (
                SetupRefinementPurpose::DrainBias,
                SetupRefinementError::Input(InputError::RetainedDiblNotApplied),
            ),
            (
                SetupRefinementPurpose::Reset,
                SetupRefinementError::Refit(RefitError::RetainedDiblNotApplied),
            ),
        ] {
            assert_eq!(
                setup_refinement_error_message(purpose, error),
                MODEL_DIBL_REAPPLY_FAILED_MESSAGE
            );
        }
    }

    #[test]
    fn fit_ready_failures_are_classified_without_refitting() {
        let mut state = ModelFitState::default();
        let mut issues = IngestIssues::default();
        let mut toasts = Toasts::default();

        apply_files_fitted(
            vec![
                (
                    PrimaryTransferSource::new("flat.csv", None).unwrap(),
                    Err(LoadError::Unfittable),
                ),
                (
                    PrimaryTransferSource::new("bad.csv", None).unwrap(),
                    Err(LoadError::Parse("missing Vg column".into())),
                ),
            ],
            &mut state,
            &mut issues,
            &mut toasts,
        );

        assert_eq!(state.device_count(), 0);
        assert_eq!(issues.rows().count(), 2);
    }

    #[test]
    fn fit_ready_batch_counts_a_canonical_alias_as_already_loaded() {
        let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let alias_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("..")
            .join("Cargo.toml");
        let mut state = ModelFitState::default();
        let mut issues = IngestIssues::default();
        let mut toasts = Toasts::default();

        apply_files_fitted(
            vec![
                (
                    PrimaryTransferSource::new("device.csv", Some(original_path.clone())).unwrap(),
                    Ok(fitted_device("device.csv", 2.0)),
                ),
                (
                    PrimaryTransferSource::new("device.csv", Some(alias_path)).unwrap(),
                    Ok(fitted_device("device.csv", 3.0)),
                ),
            ],
            &mut state,
            &mut issues,
            &mut toasts,
        );

        assert_eq!(state.device_count(), 1);
        assert_eq!(
            state.devices()[0].transfer_source_path(),
            Some(original_path.as_path())
        );
        assert_eq!(issues.rows().count(), 0);
    }

    #[test]
    fn cv_parse_failure_persists_its_file_name_in_the_model_file_list() {
        let mut state = ModelFitState::default();
        let mut issues = IngestIssues::default();
        let mut toasts = Toasts::default();

        apply_cv_parsed(
            None,
            "bad_cv.csv".into(),
            Err("missing C column".into()),
            &mut state,
            &mut issues,
            &mut toasts,
        );

        let rows = issues.rows().collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "bad_cv.csv");
        assert_eq!(rows[0].message, "missing C column");
    }

    #[test]
    fn cv_apply_failure_persists_its_file_name_in_the_model_file_list() {
        let mut state = ModelFitState::default();
        let mut issues = IngestIssues::default();
        let mut toasts = Toasts::default();

        apply_cv_parsed(
            None,
            "valid_cv.csv".into(),
            Ok(1.0e-10),
            &mut state,
            &mut issues,
            &mut toasts,
        );

        let rows = issues.rows().collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert!(matches!(
            rows[0],
            IssueRow {
                name: "valid_cv.csv",
                message: MODEL_DEVICE_REQUIRED_MESSAGE,
                ..
            }
        ));
    }
}
