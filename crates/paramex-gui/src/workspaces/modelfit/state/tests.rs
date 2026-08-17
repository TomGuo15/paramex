use super::*;

fn primary(name: &str) -> PrimaryTransferSource {
    PrimaryTransferSource::new(name, None).expect("test primary source is visibly named")
}

fn fitted_device(name: &str, vt: f64) -> FittedDevice {
    let vg: Vec<f64> = (0..=120).map(|idx| -2.0 + idx as f64 * 0.1).collect();
    let id = synthetic_transfer(
        &ModelParams {
            vt,
            gamma: 0.5,
            k: 1.0e-6,
        },
        &vg,
    );
    FittedDevice::fit(name.to_owned(), vg, id).expect("test device fits")
}

fn dibl_science() -> (DeviceScience, SecondTransfer) {
    let vg: Vec<_> = (0..=120).map(|idx| -2.0 + idx as f64 * 0.1).collect();
    let primary_id = synthetic_transfer(
        &ModelParams {
            vt: 2.0,
            gamma: 0.5,
            k: 1.0e-6,
        },
        &vg,
    );
    let second = SecondTransfer {
        id_abs: synthetic_transfer(
            &ModelParams {
                vt: 2.5,
                gamma: 0.5,
                k: 1.0e-6,
            },
            &vg,
        ),
        vg,
        v_ds: 1.0,
    };
    let device = FittedDevice::fit("high.csv".into(), second.vg.clone(), primary_id).expect("fits");
    (
        DeviceScience::new(device, primary("high.csv"), None, None).expect("transfer-only science"),
        second,
    )
}

fn add(state: &mut ModelFitState, name: &str, vt: f64) {
    let device = fitted_device(name, vt);
    assert_eq!(
        state
            .install_fitted_device(device, primary(name), None)
            .expect("test transfer has no output curves"),
        DeviceInstallOutcome::Installed
    );
}

#[test]
fn device_science_dibl_attachment_and_detachment_keep_provenance_atomic() {
    let (science, second) = dibl_science();
    let primary_source = primary("high.csv");
    let source = DiblSource::new("low.csv", None).unwrap();
    assert!(matches!(
        DeviceScience::new(
            science.device().clone(),
            primary_source.clone(),
            None,
            Some(source.clone())
        ),
        Err(DeviceInstallError::DiblSourceWithoutSecondTransfer)
    ));

    let replacement = science
        .replacing_second_transfer(source, second)
        .expect("real DIBL pair refines");
    assert!(replacement.displaced.is_none());
    let science = replacement.science;
    assert!(science.device().has_second_transfer());
    assert_eq!(science.dibl_source().map(DiblSource::name), Some("low.csv"));
    assert!(matches!(
        DeviceScience::new(science.device().clone(), primary_source, None, None),
        Err(DeviceInstallError::DiblSourceRequired)
    ));

    let (science, detached) = science
        .without_second_transfer()
        .expect("attached DIBL measurement detaches");
    assert_eq!(detached.source.name(), "low.csv");
    assert!(!science.device().has_second_transfer());
    assert!(science.dibl_source().is_none());
}

#[test]
fn device_science_returns_rejected_dibl_measurement_with_its_source_and_reason() {
    let (science, second) = dibl_science();
    let mut device = science.device().clone();
    device
        .set_level62_params(device.level62().expect("Level 62 fit").params)
        .expect("manual edit");
    let science = DeviceScience::new(device, science.primary_source().clone(), None, None)
        .expect("manual science remains coherent");
    let source = DiblSource::new("low.csv", Some("lot-a/low.csv".into())).unwrap();

    let Err(rejected) = science.replacing_second_transfer(source.clone(), second.clone()) else {
        panic!("manual Level 62 must reject DIBL replacement");
    };
    assert_eq!(rejected.source, source);
    assert_eq!(rejected.second, second);
    assert_eq!(rejected.reason, DiblError::Level62Manual);
    assert!(!science.device().has_second_transfer());
    assert!(science.dibl_source().is_none());
}

#[test]
fn different_source_dibl_replacement_returns_prior_measurement_and_provenance() {
    let (science, first) = dibl_science();
    let initial = science
        .replacing_second_transfer(
            DiblSource::new("low-a.csv", Some("lot-a/low.csv".into())).unwrap(),
            first.clone(),
        )
        .expect("first DIBL measurement fits");
    assert!(initial.displaced.is_none());

    let replacement = initial
        .science
        .replacing_second_transfer(
            DiblSource::new("low-b.csv", Some("lot-b/low.csv".into())).unwrap(),
            first.clone(),
        )
        .expect("replacement DIBL measurement fits");
    let displaced = replacement
        .displaced
        .expect("different source returns prior DIBL science");
    assert_eq!(displaced.source.name(), "low-a.csv");
    assert_eq!(displaced.second, first);
    assert_eq!(
        replacement.science.dibl_source().map(DiblSource::name),
        Some("low-b.csv")
    );
}

#[test]
fn canonical_same_source_dibl_reload_supersedes_without_displacement() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let alias = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("..")
        .join("Cargo.toml");
    let (science, second) = dibl_science();
    let initial = science
        .replacing_second_transfer(
            DiblSource::new("old-label.csv", Some(manifest)).unwrap(),
            second.clone(),
        )
        .expect("first DIBL measurement fits");

    let replacement = initial
        .science
        .replacing_second_transfer(
            DiblSource::new("new-label.csv", Some(alias)).unwrap(),
            second,
        )
        .expect("same-source reload fits");
    assert!(replacement.displaced.is_none());
    assert_eq!(
        replacement.science.dibl_source().map(DiblSource::name),
        Some("new-label.csv")
    );
}

fn device_with_output(name: &str, vt: f64) -> FittedDevice {
    let mut device = fitted_device(name, vt);
    assert!(device
        .replace_output(vec![OutputCurve {
            vg: vt + 2.0,
            vds: vec![0.0, 1.0],
            id: vec![0.0, 1.0e-6],
        }])
        .expect("device without retained DIBL accepts output")
        .displaced
        .is_empty());
    assert!(device.has_output_curves());
    device
}

#[test]
fn first_installed_row_is_selected_and_checkbox_state_is_gui_local() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    assert_eq!(state.selected_index(), Some(0));
    assert!(state.set_device_checked(0, true));
    assert!(state.devices()[0].is_checked());
}

#[test]
fn install_requires_visible_source_metadata_for_output_curves() {
    let mut state = ModelFitState::default();
    let error = state
        .install_fitted_device(device_with_output("a", 2.0), primary("a"), None)
        .unwrap_err();

    assert_eq!(error, DeviceInstallError::OutputSourceRequired);
    assert!(state.is_empty());
}

#[test]
fn install_rejects_output_source_for_transfer_only_device() {
    let mut state = ModelFitState::default();
    let source = OutputSource::new("a_output.csv", None).unwrap();
    let error = state
        .install_fitted_device(fitted_device("a", 2.0), primary("a"), Some(source))
        .unwrap_err();

    assert_eq!(error, DeviceInstallError::OutputSourceWithoutCurves);
    assert!(state.is_empty());
}

#[test]
fn installed_output_source_is_nonempty_and_visible() {
    assert_eq!(
        OutputSource::new("  ", None),
        Err(OutputSourceError::EmptyName)
    );

    let mut state = ModelFitState::default();
    let source = OutputSource::new("a_output.csv", None).unwrap();
    assert_eq!(
        state
            .install_fitted_device(device_with_output("a", 2.0), primary("a"), Some(source))
            .unwrap(),
        DeviceInstallOutcome::Installed
    );

    assert_eq!(state.devices()[0].output_name(), Some("a_output.csv"));
}

#[test]
fn primary_source_requires_a_visible_name_matching_the_fitted_device() {
    assert_eq!(
        PrimaryTransferSource::new("  ", None),
        Err(PrimaryTransferSourceError::EmptyName)
    );

    let mut state = ModelFitState::default();
    let error = state
        .install_fitted_device(
            fitted_device("device.csv", 2.0),
            PrimaryTransferSource::new("other.csv", None).unwrap(),
            None,
        )
        .unwrap_err();

    assert_eq!(error, DeviceInstallError::PrimarySourceNameMismatch);
    assert!(state.is_empty());
}

#[test]
fn primary_identity_handles_canonical_aliases_pathless_fallback_and_distinct_paths() {
    let gui_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let gui_manifest_alias = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("..")
        .join("Cargo.toml");
    let core_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("paramex-core")
        .join("Cargo.toml");

    let mut canonical = ModelFitState::default();
    assert_eq!(
        canonical
            .install_fitted_device(
                fitted_device("Cargo.toml", 2.0),
                PrimaryTransferSource::new("Cargo.toml", Some(gui_manifest.clone())).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    assert_eq!(
        canonical
            .install_fitted_device(
                fitted_device("Cargo.toml", 3.0),
                PrimaryTransferSource::new("Cargo.toml", Some(gui_manifest_alias)).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::AlreadyLoaded
    );
    assert_eq!(canonical.device_count(), 1);

    let mut pathless = ModelFitState::default();
    assert_eq!(
        pathless
            .install_fitted_device(
                fitted_device("Cargo.toml", 2.0),
                PrimaryTransferSource::new("Cargo.toml", Some(gui_manifest.clone())).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    assert_eq!(
        pathless
            .install_fitted_device(
                fitted_device("Cargo.toml", 3.0),
                PrimaryTransferSource::new("Cargo.toml", None).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::AlreadyLoaded
    );
    assert_eq!(pathless.device_count(), 1);

    let mut distinct = ModelFitState::default();
    assert_eq!(
        distinct
            .install_fitted_device(
                fitted_device("Cargo.toml", 2.0),
                PrimaryTransferSource::new("Cargo.toml", Some(gui_manifest)).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    assert_eq!(
        distinct
            .install_fitted_device(
                fitted_device("Cargo.toml", 3.0),
                PrimaryTransferSource::new("Cargo.toml", Some(core_manifest)).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    assert_eq!(distinct.device_count(), 2);
}

#[test]
fn duplicate_primary_admission_is_a_complete_no_op() {
    let original_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let alias_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("..")
        .join("Cargo.toml");
    let mut state = ModelFitState::default();
    assert_eq!(
        state
            .install_fitted_device(
                device_with_output("device.csv", 2.0),
                PrimaryTransferSource::new("device.csv", Some(original_path.clone())).unwrap(),
                Some(OutputSource::new("device_output.csv", None).unwrap()),
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );

    let vg: Vec<_> = (0..=120).map(|idx| -2.0 + idx as f64 * 0.1).collect();
    let second = SecondTransfer {
        id_abs: synthetic_transfer(
            &ModelParams {
                vt: 2.5,
                gamma: 0.5,
                k: 1.0e-6,
            },
            &vg,
        ),
        vg,
        v_ds: 1.0,
    };
    let replacement = state.devices[0]
        .science()
        .replacing_second_transfer(DiblSource::new("device_low.csv", None).unwrap(), second)
        .expect("second transfer refines");
    assert!(replacement.displaced.is_none());
    state.devices[0].commit_science(replacement.science);
    assert!(state.set_device_checked(0, true));

    add(&mut state, "selected.csv", 3.0);
    state.select(1);
    let plan = state.plan_output_imports(vec![OutputImport {
        source: OutputSource::new("orphan_output.csv", None).unwrap(),
        curves: Vec::new(),
    }]);
    state.commit_output_refinement(run_output_refinement(plan));

    let before_id = state.devices[0].id();
    let before_revision = state.devices[0].revision();
    let before_device = state.devices[0].device().clone();
    let before_next_id = state.next_device_id;
    let before_pending = state
        .pending_outputs()
        .iter()
        .map(|pending| {
            (
                pending.name().to_owned(),
                pending.source_path().map(Path::to_path_buf),
                pending.reason(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        state
            .install_fitted_device(
                fitted_device("device.csv", 4.0),
                PrimaryTransferSource::new("device.csv", Some(alias_path)).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::AlreadyLoaded
    );

    assert_eq!(state.device_count(), 2);
    assert_eq!(state.selected_index(), Some(1));
    assert_eq!(state.next_device_id, before_next_id);
    let original = &state.devices[0];
    assert_eq!(original.id(), before_id);
    assert_eq!(original.revision(), before_revision);
    assert_eq!(original.device(), &before_device);
    assert!(original.is_checked());
    assert_eq!(
        original.transfer_source_path(),
        Some(original_path.as_path())
    );
    assert_eq!(original.output_name(), Some("device_output.csv"));
    assert_eq!(original.dibl_name(), Some("device_low.csv"));
    assert_eq!(
        state
            .pending_outputs()
            .iter()
            .map(|pending| {
                (
                    pending.name().to_owned(),
                    pending.source_path().map(Path::to_path_buf),
                    pending.reason(),
                )
            })
            .collect::<Vec<_>>(),
        before_pending
    );
}

#[test]
fn guarded_worker_commit_preserves_primary_provenance() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let mut state = ModelFitState::default();
    assert_eq!(
        state
            .install_fitted_device(
                fitted_device("device.csv", 2.0),
                PrimaryTransferSource::new("device.csv", Some(source_path.clone())).unwrap(),
                None,
            )
            .unwrap(),
        DeviceInstallOutcome::Installed
    );
    let plan = state
        .plan_selected_setup(SetupOperation::Geometry(GeometryParams {
            w_um: 42.0,
            l_um: 7.0,
        }))
        .unwrap()
        .unwrap();

    state.commit_setup_refinement(run_setup_refinement(plan));

    assert_eq!(
        state.devices[0].transfer_source_path(),
        Some(source_path.as_path())
    );
    assert_eq!(state.devices[0].transfer_name(), "device.csv");
}

#[test]
fn dibl_source_requires_a_visible_name() {
    assert_eq!(DiblSource::new("  ", None), Err(DiblSourceError::EmptyName));
    let source = DiblSource::new("low_vds.csv", Some("lot/low_vds.csv".into())).unwrap();
    assert_eq!(source.name(), "low_vds.csv");
    assert_eq!(source.path(), Some(Path::new("lot/low_vds.csv")));
}

#[test]
fn invalid_row_selection_and_removal_are_inert() {
    let mut state = ModelFitState::default();
    state.select(0);
    assert_eq!(state.selected_index(), None);
    assert!(!state.remove_device(0));
}

#[test]
fn clear_drops_rows_pending_files_and_selection() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    let plan = state.plan_output_imports(vec![OutputImport {
        source: OutputSource::new("orphan_output.csv", None).unwrap(),
        curves: Vec::new(),
    }]);
    state.commit_output_refinement(run_output_refinement(plan));
    state.add_pending_dibl(
        DiblSource::new("orphan_dibl.csv", None).unwrap(),
        SecondTransfer {
            vg: vec![0.0; 12],
            id_abs: vec![1.0e-9; 12],
            v_ds: 1.0,
        },
        PendingDiblReason::NoMatch,
    );
    state.clear();
    assert!(state.is_empty());
    assert!(state.pending_outputs().is_empty());
    assert!(state.pending_dibls().is_empty());
    assert_eq!(state.selected_index(), None);
}

#[test]
fn removing_checked_rows_keeps_the_selected_survivor() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    add(&mut state, "b", 2.0);
    state.select(1);
    assert!(state.set_device_checked(0, true));
    assert_eq!(state.remove_selected_or_checked(), 1);
    assert_eq!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some("b")
    );
}

#[test]
fn removing_selected_row_selects_the_first_survivor() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    add(&mut state, "b", 2.0);
    state.select(1);
    assert_eq!(state.remove_selected_or_checked(), 1);
    assert_eq!(
        state.selected_entry().map(|entry| entry.device().name()),
        Some("a")
    );
}

#[test]
fn keep_checked_removes_only_unchecked_rows() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    add(&mut state, "b", 2.0);
    assert!(state.set_device_checked(1, true));
    assert_eq!(state.keep_checked_devices(), Some(1));
    assert_eq!(state.devices().len(), 1);
    assert_eq!(state.devices()[0].device().name(), "b");
}

#[test]
fn keep_checked_requires_a_checked_row() {
    let mut state = ModelFitState::default();
    add(&mut state, "a", 2.0);
    assert_eq!(state.keep_checked_devices(), None);
}

#[test]
fn model_menu_only_accepts_registry_entries() {
    let mut state = ModelFitState::default();
    assert!(state.set_selected_model(LEVEL62_INDEX));
    assert!(!state.set_selected_model(FIT_MODELS.len()));
}

#[test]
fn selected_model_remains_a_gui_menu_index() {
    let mut state = ModelFitState::default();
    assert_eq!(state.selected_model(), AOSTFT_INDEX);
    assert!(state.set_selected_model(LEVEL62_INDEX));
    assert_eq!(state.selected_fit_model(), FitModel::Level62);
}

#[test]
fn selected_mutations_require_a_selected_row() {
    let mut state = ModelFitState::default();
    assert_eq!(
        state.set_selected_cox(1.0e-4),
        Err(SelectedMutationError::NoDeviceSelected)
    );
    assert_eq!(
        state.commit_cox_from_cv(None, 1.0e-12),
        Err(CvCommitError::Mutation(
            SelectedMutationError::NoDeviceSelected
        ))
    );
}

#[test]
fn cv_commit_uses_the_click_time_device_not_the_current_selection() {
    let mut state = ModelFitState::default();
    add(&mut state, "first.csv", 1.0);
    add(&mut state, "second.csv", 2.0);
    let target = state.devices[0].token();
    let target_revision = state.devices[0].revision().get();
    let other_revision = state.devices[1].revision();
    state.select(1);

    assert!(state.commit_cox_from_cv(Some(target), 1.0e-10).is_ok());

    assert_eq!(
        state.devices[0].revision().get(),
        target_revision + 1,
        "C-V science commits to the row captured at click time"
    );
    assert_eq!(
        state.devices[1].revision(),
        other_revision,
        "selection changes must not redirect C-V science"
    );
}

#[test]
fn cv_commit_rejects_a_scientifically_changed_target() {
    let mut state = ModelFitState::default();
    add(&mut state, "first.csv", 1.0);
    let target = state.devices[0].token();
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.25, fit.gamma, fit.k)
        .unwrap();
    let edited_revision = state.devices[0].revision();

    assert_eq!(
        state.commit_cox_from_cv(Some(target), 1.0e-10),
        Err(CvCommitError::DeviceChanged)
    );
    assert_eq!(state.devices[0].revision(), edited_revision);
}

#[test]
fn cv_commit_rejects_a_removed_click_time_target() {
    let mut state = ModelFitState::default();
    add(&mut state, "first.csv", 1.0);
    let target = state.selected_token();
    assert!(state.remove_device(0));

    assert_eq!(
        state.commit_cox_from_cv(target, 1.0e-10),
        Err(CvCommitError::DeviceChanged)
    );
    assert!(state.devices().is_empty());
}

#[test]
fn unchanged_cox_does_not_advance_the_scientific_revision() {
    let mut state = ModelFitState::default();
    add(&mut state, "first.csv", 1.0);
    let entry = state.selected_entry().unwrap();
    let cox = entry.device().bias().cox;
    let revision = entry.revision();

    state.set_selected_cox(cox).unwrap();

    assert_eq!(state.selected_entry().unwrap().revision(), revision);
}

#[test]
fn selected_cox_update_preserves_dc_fit_state() {
    let mut state = ModelFitState::default();
    state.load_demo();
    let entry = state.selected_entry().unwrap();
    let fit = *entry.device().aostft_fit();
    let output = entry.device().output();
    let level62 = entry.device().level62().cloned();
    let revision = entry.revision();

    state.set_selected_cox(2.0e-4).unwrap();

    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.device().bias().cox, 2.0e-4);
    assert_eq!(*entry.device().aostft_fit(), fit);
    assert_eq!(entry.device().output(), output);
    assert_eq!(entry.device().level62(), level62.as_ref());
    assert_eq!(entry.revision().get(), revision.get() + 1);
}
