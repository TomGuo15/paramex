//! Geometry, drain-bias, and auto-fit reset planning, worker, and guarded commit.

use paramex_core::modelfit::{FitModel, GeometryParams, InputError, RefitError};

use super::super::{DeviceScience, DeviceToken, ModelFitState, SelectedMutationError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SetupOperation {
    Geometry(GeometryParams),
    DrainBias(f64),
    Reset(FitModel),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetupRefinementPurpose {
    Geometry,
    DrainBias,
    Reset,
}

impl SetupOperation {
    fn purpose(self) -> SetupRefinementPurpose {
        match self {
            Self::Geometry(_) => SetupRefinementPurpose::Geometry,
            Self::DrainBias(_) => SetupRefinementPurpose::DrainBias,
            Self::Reset(_) => SetupRefinementPurpose::Reset,
        }
    }
}

pub(crate) struct SetupRefinementPlan {
    token: DeviceToken,
    device_name: String,
    science: DeviceScience,
    operation: SetupOperation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetupRefinementError {
    Input(InputError),
    Refit(RefitError),
}

pub(crate) struct SetupRefinementResult {
    token: DeviceToken,
    device_name: String,
    purpose: SetupRefinementPurpose,
    result: Result<DeviceScience, SetupRefinementError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SetupCommitOutcome {
    Applied,
    Rejected(SetupRefinementError),
    Stale,
}

pub(crate) struct SetupCommitReport {
    pub(crate) device_name: String,
    pub(crate) purpose: SetupRefinementPurpose,
    pub(crate) outcome: SetupCommitOutcome,
}

impl ModelFitState {
    pub(crate) fn plan_selected_setup(
        &self,
        operation: SetupOperation,
    ) -> Result<Option<SetupRefinementPlan>, SelectedMutationError> {
        let entry = self
            .selected_entry()
            .ok_or(SelectedMutationError::NoDeviceSelected)?;
        let unchanged = match operation {
            SetupOperation::Geometry(geometry) => entry.device().geometry() == geometry,
            SetupOperation::DrainBias(v_ds) => entry.device().bias().v_ds == v_ds,
            SetupOperation::Reset(model) => !entry.device().model(model).is_manual(),
        };
        Ok((!unchanged).then(|| SetupRefinementPlan {
            token: entry.token(),
            device_name: entry.device().name().to_owned(),
            science: entry.science().clone(),
            operation,
        }))
    }

    pub(crate) fn commit_setup_refinement(
        &mut self,
        result: SetupRefinementResult,
    ) -> SetupCommitReport {
        let purpose = result.purpose;
        let device_name = result.device_name;
        let Some(entry) = self
            .devices
            .iter_mut()
            .find(|entry| entry.id == result.token.id && entry.revision == result.token.revision)
        else {
            return SetupCommitReport {
                device_name,
                purpose,
                outcome: SetupCommitOutcome::Stale,
            };
        };
        match result.result {
            Ok(science) => {
                entry.commit_science(science);
                SetupCommitReport {
                    device_name,
                    purpose,
                    outcome: SetupCommitOutcome::Applied,
                }
            }
            Err(error) => SetupCommitReport {
                device_name,
                purpose,
                outcome: SetupCommitOutcome::Rejected(error),
            },
        }
    }
}

pub(crate) fn run_setup_refinement(plan: SetupRefinementPlan) -> SetupRefinementResult {
    let SetupRefinementPlan {
        token,
        device_name,
        mut science,
        operation,
    } = plan;
    let purpose = operation.purpose();
    let result = match operation {
        SetupOperation::Geometry(geometry) => science
            .set_geometry(geometry)
            .map_err(SetupRefinementError::Input),
        SetupOperation::DrainBias(v_ds) => science
            .set_drain_bias(v_ds)
            .map_err(SetupRefinementError::Input),
        SetupOperation::Reset(model) => science
            .reset_autofit(model)
            .map_err(SetupRefinementError::Refit),
    }
    .map(|()| science);
    SetupRefinementResult {
        token,
        device_name,
        purpose,
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::super::add_test_device as add_device;
    use super::super::output::{run_output_refinement, OutputImport};
    use super::*;
    use crate::workspaces::modelfit::state::OutputSource;
    use paramex_core::modelfit::OutputCurve;

    fn output(name: &str) -> OutputImport {
        OutputImport {
            source: OutputSource::new(name, Some(name.into())).unwrap(),
            curves: vec![OutputCurve {
                vg: 4.0,
                vds: vec![0.0, 1.0],
                id: vec![0.0, 1.0e-6],
            }],
        }
    }

    #[test]
    fn setup_worker_commits_one_clone_without_replacing_gui_metadata() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev_transfer.csv", 1.0);
        assert!(state.set_device_checked(0, true));
        let plan = state.plan_output_imports(vec![output("dev_output.csv")]);
        state.commit_output_refinement(run_output_refinement(plan));
        let revision = state.devices[0].revision();
        let plan = state
            .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
                w_um: 42.0,
                l_um: 7.0,
            }))
            .unwrap()
            .expect("changed geometry plans a worker");

        let report = state.commit_setup_refinement(run_setup_refinement(plan));

        assert_eq!(report.outcome, SetupCommitOutcome::Applied);
        let entry = &state.devices[0];
        assert_eq!(
            entry.device().geometry(),
            GeometryParams {
                w_um: 42.0,
                l_um: 7.0
            }
        );
        assert_eq!(entry.revision().get(), revision.get() + 1);
        assert!(entry.is_checked());
        assert_eq!(entry.output_name(), Some("dev_output.csv"));
    }

    #[test]
    fn setup_worker_rejection_is_atomic_and_typed() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev_transfer.csv", 1.0);
        let before = state.devices[0].device().clone();
        let revision = state.devices[0].revision();
        let plan = state
            .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
                w_um: 0.0,
                l_um: 1.0,
            }))
            .unwrap()
            .expect("syntax validation belongs to the UI boundary");

        let report = state.commit_setup_refinement(run_setup_refinement(plan));

        assert_eq!(
            report.outcome,
            SetupCommitOutcome::Rejected(SetupRefinementError::Input(InputError::InvalidGeometry))
        );
        assert_eq!(state.devices[0].device(), &before);
        assert_eq!(state.devices[0].revision(), revision);
    }

    #[test]
    fn setup_commit_follows_identity_across_index_reorder() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "first.csv", 1.0);
        add_device(&mut state, "second.csv", 2.0);
        state.select(1);
        let target = state.selected_device_id().unwrap();
        let plan = state
            .plan_selected_setup(SetupOperation::DrainBias(0.25))
            .unwrap()
            .expect("changed VDS");
        let result = run_setup_refinement(plan);

        assert!(state.remove_device(0));
        let report = state.commit_setup_refinement(result);

        assert_eq!(report.outcome, SetupCommitOutcome::Applied);
        let target = state
            .devices()
            .iter()
            .find(|entry| entry.id() == target)
            .expect("target survives reorder");
        assert_eq!(target.device().bias().v_ds, 0.25);
    }

    #[test]
    fn stale_setup_wins_over_an_obsolete_worker_rejection() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev.csv", 1.0);
        let plan = state
            .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
                w_um: 0.0,
                l_um: 1.0,
            }))
            .unwrap()
            .unwrap();
        let result = run_setup_refinement(plan);
        assert!(state.remove_device(0));

        let report = state.commit_setup_refinement(result);

        assert_eq!(report.outcome, SetupCommitOutcome::Stale);
    }

    #[test]
    fn scientific_edit_makes_setup_result_stale_without_overwriting_live_state() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev.csv", 1.0);
        let plan = state
            .plan_selected_setup(SetupOperation::DrainBias(0.25))
            .unwrap()
            .unwrap();
        let result = run_setup_refinement(plan);
        let fit = *state.selected_entry().unwrap().device().aostft_fit();
        state
            .set_selected_fit(fit.vt + 0.5, fit.gamma, fit.k)
            .unwrap();
        let edited = state.selected_entry().unwrap().device().clone();

        let report = state.commit_setup_refinement(result);

        assert_eq!(report.outcome, SetupCommitOutcome::Stale);
        assert_eq!(state.selected_entry().unwrap().device(), &edited);
    }

    #[test]
    fn setup_planning_skips_numerically_unchanged_values() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev.csv", 1.0);
        let device = state.selected_entry().unwrap().device();
        assert!(state
            .plan_selected_setup(SetupOperation::Geometry(device.geometry()))
            .unwrap()
            .is_none());
        assert!(state
            .plan_selected_setup(SetupOperation::DrainBias(device.bias().v_ds))
            .unwrap()
            .is_none());
        assert!(state
            .plan_selected_setup(SetupOperation::Reset(FitModel::Aostft))
            .unwrap()
            .is_none());
    }

    #[test]
    fn reset_worker_clears_manual_mode_only_after_guarded_commit() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev.csv", 1.0);
        let fit = *state.selected_entry().unwrap().device().aostft_fit();
        state
            .set_selected_fit(fit.vt + 0.5, fit.gamma, fit.k)
            .unwrap();
        let plan = state
            .plan_selected_setup(SetupOperation::Reset(FitModel::Aostft))
            .unwrap()
            .expect("manual model needs reset");
        assert!(state.is_selected_manual(FitModel::Aostft));

        let report = state.commit_setup_refinement(run_setup_refinement(plan));

        assert_eq!(report.outcome, SetupCommitOutcome::Applied);
        assert!(!state.is_selected_manual(FitModel::Aostft));
    }
}
