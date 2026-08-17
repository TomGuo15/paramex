//! Guarded Model Fit worker workflows.
//!
//! Each private child module owns one concrete plan → worker → current-token
//! commit flow. Stable identity and scientific revision remain shared in the
//! parent state module; no generic refinement framework sits between them.

mod dibl;
mod output;
mod pending;
mod setup;

#[cfg(test)]
pub(crate) use dibl::DiblFit;
pub(crate) use dibl::{
    run_dibl_refinement, DiblCommitReport, DiblImport, DiblIssue, DiblIssueKind,
    DiblRefinementMode, DiblRefinementPlan, DiblRefinementPurpose, DiblRefinementRecovery,
    DiblRefinementResult,
};
pub(crate) use output::{
    run_output_refinement, OutputImport, OutputIssue, OutputRefinementPlan,
    OutputRefinementPurpose, OutputRefinementRecovery, OutputRefinementResult,
};
pub(crate) use setup::{
    run_setup_refinement, SetupCommitOutcome, SetupOperation, SetupRefinementError,
    SetupRefinementPlan, SetupRefinementPurpose, SetupRefinementResult,
};

#[cfg(test)]
fn add_test_device(state: &mut super::ModelFitState, name: &str, vt: f64) {
    use paramex_core::modelfit::{FittedDevice, ModelParams};

    let params = ModelParams {
        vt,
        gamma: 0.5,
        k: 1.0e-6,
    };
    let vgs = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
    let id = super::synthetic_transfer(&params, &vgs);
    let device = FittedDevice::fit(name.to_owned(), vgs, id).expect("test device fits");
    assert_eq!(
        state
            .install_fitted_device(
                device,
                super::PrimaryTransferSource::new(name, None).unwrap(),
                None,
            )
            .expect("test transfer has no output curves"),
        super::DeviceInstallOutcome::Installed
    );
}

#[cfg(test)]
mod tests {
    use super::super::ModelFitState;
    use super::add_test_device as add_device;

    #[test]
    fn identities_are_monotonic_and_only_scientific_mutations_bump_revision() {
        let mut state = ModelFitState::default();
        add_device(&mut state, "a_transfer.csv", 1.0);
        add_device(&mut state, "b_transfer.csv", 2.0);
        let first_id = state.devices[0].id();
        let second_id = state.devices[1].id();
        assert!(second_id.get() > first_id.get());
        let first_revision = state.devices[0].revision();

        state.select(1);
        state.set_device_checked(0, true);
        assert!(state.set_selected_model(1));
        assert_eq!(state.devices[0].revision(), first_revision);
        assert_eq!(state.devices[1].revision().get(), 0);

        state.select(0);
        let fit = *state.selected_entry().unwrap().device().aostft_fit();
        assert!(state
            .set_selected_fit(fit.vt + 0.25, fit.gamma, fit.k)
            .is_ok());
        assert_eq!(state.devices[0].revision().get(), first_revision.get() + 1);
        let changed_revision = state.devices[0].revision();
        assert!(state.set_selected_cox(f64::NAN).is_err());
        assert_eq!(state.devices[0].revision(), changed_revision);

        state.clear();
        add_device(&mut state, "c_transfer.csv", 3.0);
        assert!(state.devices[0].id().get() > second_id.get());
    }
}
