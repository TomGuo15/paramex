use std::sync::{Mutex, OnceLock};

use super::super::add_test_device as add_device;
use super::*;
use crate::workspaces::modelfit::state::{synthetic_transfer, DiblSource, PrimaryTransferSource};
use paramex_core::modelfit::{FitModel, ModelParams, SecondTransfer};

fn output(name: &str, path: &str) -> OutputImport {
    output_at(name, path.into())
}

fn output_at(name: &str, path: std::path::PathBuf) -> OutputImport {
    OutputImport {
        source: OutputSource::new(name, Some(path)).unwrap(),
        curves: vec![OutputCurve {
            vg: 4.0,
            vds: vec![0.0, 1.0],
            id: vec![0.0, 1.0e-6],
        }],
    }
}

fn state_with_output_and_dibl() -> (ModelFitState, Vec<OutputCurve>) {
    static FIXTURE: OnceLock<Mutex<(FittedDevice, Vec<OutputCurve>)>> = OnceLock::new();
    let (device, output) = FIXTURE
        .get_or_init(|| {
            let params = ModelParams {
                vt: 1.0,
                gamma: 0.5,
                k: 1.0e-6,
            };
            let vg = (0..=100).map(|idx| idx as f64 * 0.1).collect::<Vec<_>>();
            let id = synthetic_transfer(&params, &vg);
            let mut device = FittedDevice::fit("dev_transfer.csv".to_owned(), vg.clone(), id)
                .expect("synthetic device fits");
            let output = device
                .model(FitModel::Level62)
                .output_family()
                .into_iter()
                .map(|series| OutputCurve {
                    vg: series.vg,
                    vds: series.modelled.iter().map(|point| point[0]).collect(),
                    id: series.modelled.iter().map(|point| point[1]).collect(),
                })
                .collect::<Vec<_>>();
            assert!(!output.is_empty(), "Level 62 predicts an output family");
            assert!(device
                .replace_output(output.clone())
                .expect("model-consistent output attaches")
                .displaced
                .is_empty());
            let second_params = ModelParams { vt: 1.5, ..params };
            assert!(device
                .replace_second_transfer(SecondTransfer {
                    id_abs: synthetic_transfer(&second_params, &vg),
                    vg,
                    v_ds: 1.0,
                })
                .expect("synthetic DIBL measurement fits")
                .displaced
                .is_none());
            Mutex::new((device, output))
        })
        .lock()
        .expect("output/DIBL fixture cache remains available")
        .clone();

    let science = DeviceScience::new(
        device,
        PrimaryTransferSource::new("dev_transfer.csv", None).unwrap(),
        Some(OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap()),
        Some(DiblSource::new("dev_low.csv", Some("lot/dev_low.csv".into())).unwrap()),
    )
    .expect("fixture science and provenance agree");
    let mut state = ModelFitState::default();
    state.devices.push(DeviceEntry::new(DeviceId(0), science));
    state.next_device_id = 1;
    state.selected = Some(0);
    (state, output)
}

fn conflicting_output(mut output: Vec<OutputCurve>) -> Vec<OutputCurve> {
    for curve in &mut output {
        for id in &mut curve.id {
            *id *= 1.0e-6;
        }
    }
    output
}

#[test]
fn retained_dibl_clear_error_uses_resolution_copy() {
    assert_eq!(
        output_clear_error_message(DetachOutputError::RetainedDiblNotApplied),
        MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE
    );
}

#[test]
fn panic_recovery_keeps_every_output_input_and_parse_issue_without_mutating_science() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let revision = state.selected_entry().unwrap().revision();
    let plan = state.plan_output_imports_with_issues(
        vec![
            output("dev_output.csv", "lot-a/dev_output.csv"),
            output("dev_id-vd.csv", "lot-b/dev_id-vd.csv"),
        ],
        vec![OutputIssue {
            name: "bad-output.csv".into(),
            message: "missing Vd column".into(),
            persist: true,
        }],
    );

    let report = state.recover_output_refinement(plan.panic_recovery());

    assert_eq!(report.purpose, OutputRefinementPurpose::Import);
    assert_eq!(report.recovered, 2);
    assert_eq!(report.parse_issues.len(), 1);
    assert_eq!(state.selected_entry().unwrap().revision(), revision);
    assert_eq!(state.selected_entry().unwrap().output_name(), None);
    assert_eq!(
        state
            .pending_outputs()
            .iter()
            .map(|pending| (pending.name(), pending.reason()))
            .collect::<Vec<_>>(),
        vec![
            ("dev_output.csv", PendingOutputReason::WorkerFailed),
            ("dev_id-vd.csv", PendingOutputReason::WorkerFailed),
        ]
    );
}

#[test]
fn panic_recovery_updates_pending_output_reason_without_attaching() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let pending = output("dev_output.csv", "lot/dev_output.csv");
    state.add_pending_output(pending.source, pending.curves, PendingOutputReason::NoMatch);
    let revision = state.selected_entry().unwrap().revision();
    let plan = state
        .plan_pending_output_attach(0)
        .expect("selected device and pending output");

    let report = state.recover_output_refinement(plan.panic_recovery());

    assert_eq!(report.purpose, OutputRefinementPurpose::AttachPending);
    assert_eq!(report.recovered, 1);
    assert_eq!(state.selected_entry().unwrap().revision(), revision);
    assert_eq!(state.selected_entry().unwrap().output_name(), None);
    assert_eq!(
        state.pending_outputs()[0].reason(),
        PendingOutputReason::WorkerFailed
    );
}

#[test]
fn panic_recovery_preserves_later_deferred_reason_across_canonical_aliases() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let direct_path = crate_root.join("Cargo.toml");
    let alias_path = crate_root.join("src").join("..").join("Cargo.toml");
    let latest = output_at("orphan_output.csv", alias_path);
    let latest_curves = latest.curves.clone();
    let plan = state.plan_output_imports(vec![output_at("dev_output.csv", direct_path), latest]);

    let report = state.recover_output_refinement(plan.panic_recovery());

    assert_eq!(report.recovered, 1);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::NoMatch
    );
    assert_eq!(state.pending_outputs[0].curves, latest_curves);
}

#[test]
fn panic_recovery_counts_the_generation_ordered_output_match_set() {
    fn import(path: Option<&str>, marker: f64) -> OutputImport {
        OutputImport {
            source: OutputSource::new("shared_output.csv", path.map(std::path::PathBuf::from))
                .unwrap(),
            curves: vec![OutputCurve {
                vg: marker,
                vds: vec![0.0, 1.0],
                id: vec![0.0, marker * 1.0e-6],
            }],
        }
    }

    fn recover(imports: Vec<OutputImport>) -> (ModelFitState, usize) {
        let mut state = ModelFitState::default();
        let recovery = OutputRefinementRecovery {
            purpose: OutputRefinementPurpose::Import,
            pending: imports
                .into_iter()
                .enumerate()
                .map(|(ordinal, import)| DeferredOutput {
                    ordinal,
                    import,
                    reason: PendingOutputReason::WorkerFailed,
                })
                .collect(),
            parse_issues: Vec::new(),
        };
        let recovered = state.recover_output_refinement(recovery).recovered;
        (state, recovered)
    }

    let (latest_pathless, recovered) = recover(vec![
        import(Some("lot-a/shared_output.csv"), 1.0),
        import(Some("lot-b/shared_output.csv"), 2.0),
        import(None, 3.0),
    ]);
    assert_eq!(recovered, 1);
    assert_eq!(latest_pathless.pending_outputs.len(), 1);
    assert_eq!(latest_pathless.pending_outputs[0].source.path(), None);
    assert_eq!(latest_pathless.pending_outputs[0].curves[0].vg, 3.0);

    let (latest_pathful, recovered) = recover(vec![
        import(None, 3.0),
        import(Some("lot-a/shared_output.csv"), 1.0),
        import(Some("lot-b/shared_output.csv"), 2.0),
    ]);
    assert_eq!(recovered, 2);
    assert_eq!(
        latest_pathful
            .pending_outputs
            .iter()
            .map(|pending| pending.source.path())
            .collect::<Vec<_>>(),
        vec![
            Some(std::path::Path::new("lot-a/shared_output.csv")),
            Some(std::path::Path::new("lot-b/shared_output.csv")),
        ]
    );
}

#[test]
fn panic_recovery_for_output_clear_purposes_leaves_attachment_unchanged() {
    for purpose in [
        OutputRefinementPurpose::Detach,
        OutputRefinementPurpose::Remove,
    ] {
        let mut state = ModelFitState::default();
        add_device(&mut state, "dev_transfer.csv", 1.0);
        let plan = state.plan_output_imports(vec![output("dev_output.csv", "lot/dev_output.csv")]);
        state.commit_output_refinement(run_output_refinement(plan));
        let revision = state.selected_entry().unwrap().revision();
        let plan = state
            .plan_output_clear(0, purpose)
            .expect("attached output plans a clear");

        let report = state.recover_output_refinement(plan.panic_recovery());

        assert_eq!(report.purpose, purpose);
        assert_eq!(report.recovered, 0);
        assert_eq!(state.selected_entry().unwrap().revision(), revision);
        assert_eq!(
            state.selected_entry().unwrap().output_name(),
            Some("dev_output.csv")
        );
        assert!(state.pending_outputs().is_empty());
    }
}

#[test]
fn retained_dibl_rejection_keeps_exact_output_pending_without_revision_change() {
    let (mut state, output) = state_with_output_and_dibl();
    let rejected = conflicting_output(output);
    let before = state.selected_entry().unwrap().device().clone();
    let revision = state.selected_entry().unwrap().revision();
    let plan = state.plan_output_imports(vec![OutputImport {
        source: OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
        curves: rejected.clone(),
    }]);

    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert!(!report.action_succeeded);
    assert_eq!(report.attached, 0);
    assert_eq!(report.unfittable, 0);
    assert_eq!(report.issues.len(), 1);
    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.revision(), revision);
    assert_eq!(entry.device(), &before);
    assert_eq!(entry.output_name(), Some("dev_output.csv"));
    assert_eq!(entry.dibl_name(), Some("dev_low.csv"));
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::DiblConflict
    );
    assert_eq!(state.pending_outputs[0].curves, rejected);
}

#[test]
fn newer_rejected_output_beats_older_displacement_in_a_mixed_batch() {
    let (mut state, valid) = state_with_output_and_dibl();
    let rejected = conflicting_output(valid.clone());
    let revision = state.selected_entry().unwrap().revision();
    let plan = state.plan_output_imports(vec![
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
            curves: rejected.clone(),
        },
        OutputImport {
            source: OutputSource::new("dev_id-vd.csv", Some("lot/dev_id-vd.csv".into())).unwrap(),
            curves: valid,
        },
    ]);

    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert!(report.action_succeeded);
    assert_eq!(report.displaced, 1);
    assert_eq!(
        state.selected_entry().unwrap().revision().get(),
        revision.get() + 1
    );
    assert_eq!(
        state.selected_entry().unwrap().output_name(),
        Some("dev_id-vd.csv")
    );
    assert_eq!(
        state.selected_entry().unwrap().dibl_name(),
        Some("dev_low.csv")
    );
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::DiblConflict
    );
    assert_eq!(state.pending_outputs[0].curves, rejected);
}

#[test]
fn later_successful_same_source_output_suppresses_an_earlier_rejection() {
    let (mut state, valid) = state_with_output_and_dibl();
    let plan = state.plan_output_imports(vec![
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
            curves: conflicting_output(valid.clone()),
        },
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
            curves: valid,
        },
    ]);

    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert!(report.action_succeeded);
    assert_eq!(state.pending_outputs.len(), 0);
    assert_eq!(
        state.selected_entry().unwrap().output_name(),
        Some("dev_output.csv")
    );
}

#[test]
fn later_unmatched_alias_beats_an_earlier_rejected_output_globally() {
    let (mut state, valid) = state_with_output_and_dibl();
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let direct_path = crate_root.join("Cargo.toml");
    let alias_path = crate_root.join("src").join("..").join("Cargo.toml");
    let latest = valid
        .iter()
        .cloned()
        .map(|mut curve| {
            for id in &mut curve.id {
                *id *= 0.5;
            }
            curve
        })
        .collect::<Vec<_>>();
    let plan = state.plan_output_imports(vec![
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some(direct_path)).unwrap(),
            curves: conflicting_output(valid),
        },
        OutputImport {
            source: OutputSource::new("orphan_output.csv", Some(alias_path)).unwrap(),
            curves: latest.clone(),
        },
    ]);

    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert_eq!(report.unmatched, 1);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::NoMatch
    );
    assert_eq!(state.pending_outputs[0].curves, latest);
}

#[test]
fn deferred_unmatched_and_ambiguous_outputs_keep_global_order_on_commit_and_recovery() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dup_transfer.csv", 1.0);
    add_device(&mut state, "dup_id-vg.csv", 1.5);
    let shared_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let ambiguous = || output_at("dup_output.csv", shared_path.clone());
    let unmatched = || output_at("missing_output.csv", shared_path.clone());

    let plan = state.plan_output_imports(vec![ambiguous(), unmatched()]);
    state.commit_output_refinement(run_output_refinement(plan));
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::NoMatch
    );

    assert!(state.remove_pending_output(0));
    let plan = state.plan_output_imports(vec![unmatched(), ambiguous()]);
    state.commit_output_refinement(run_output_refinement(plan));
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::Ambiguous
    );

    assert!(state.remove_pending_output(0));
    let plan = state.plan_output_imports(vec![ambiguous(), unmatched()]);
    let report = state.recover_output_refinement(plan.panic_recovery());
    assert_eq!(report.recovered, 1);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::NoMatch
    );

    assert!(state.remove_pending_output(0));
    let plan = state.plan_output_imports(vec![unmatched(), ambiguous()]);
    state.recover_output_refinement(plan.panic_recovery());
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::Ambiguous
    );
}

#[test]
fn latest_output_payload_wins_across_grouped_jobs_and_a_stale_success() {
    let (mut state, valid) = state_with_output_and_dibl();
    add_device(&mut state, "other_transfer.csv", 2.0);
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let direct_path = crate_root.join("Cargo.toml");
    let alias_path = crate_root.join("src").join("..").join("Cargo.toml");
    let latest = conflicting_output(valid.clone())
        .into_iter()
        .map(|mut curve| {
            for id in &mut curve.id {
                *id *= 0.5;
            }
            curve
        })
        .collect::<Vec<_>>();
    let other = output_at("other_output.csv", direct_path.clone());
    let other_curves = other.curves.clone();
    let plan = state.plan_output_imports(vec![
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some(direct_path.clone())).unwrap(),
            curves: conflicting_output(valid),
        },
        other,
        OutputImport {
            source: OutputSource::new("dev_output.csv", Some(alias_path)).unwrap(),
            curves: latest.clone(),
        },
    ]);
    let result = run_output_refinement(plan);
    state.select(1);
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.1, fit.gamma, fit.k)
        .expect("other-device edit makes its worker result stale");

    let report = state.commit_output_refinement(result);

    assert_eq!(report.issues.len(), 3);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::DiblConflict
    );
    assert_eq!(state.pending_outputs[0].curves, latest);
    assert_ne!(state.pending_outputs[0].curves, other_curves);
}

#[test]
fn stale_output_result_keeps_rejected_input_as_device_changed() {
    let (mut state, valid) = state_with_output_and_dibl();
    let rejected = conflicting_output(valid);
    let plan = state.plan_output_imports(vec![OutputImport {
        source: OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
        curves: rejected.clone(),
    }]);
    let result = run_output_refinement(plan);
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.1, fit.gamma, fit.k)
        .expect("scientific edit makes worker result stale");

    let report = state.commit_output_refinement(result);

    assert!(!report.action_succeeded);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::DeviceChanged
    );
    assert_eq!(state.pending_outputs[0].curves, rejected);
}

#[test]
fn existing_newer_pending_output_survives_detaching_the_older_attachment() {
    let (mut state, output) = state_with_output_and_dibl();
    let pending = conflicting_output(output);
    state.add_pending_output(
        OutputSource::new("dev_output.csv", Some("lot/dev_output.csv".into())).unwrap(),
        pending.clone(),
        PendingOutputReason::DiblConflict,
    );
    state
        .selected_entry_mut()
        .unwrap()
        .science
        .device
        .detach_second_transfer()
        .expect("removing DIBL lets output detach");
    state.selected_entry_mut().unwrap().science.dibl_source = None;
    let plan = state
        .plan_output_clear(0, OutputRefinementPurpose::Detach)
        .expect("attached output plans a detach");

    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert!(report.action_succeeded);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason,
        PendingOutputReason::DiblConflict
    );
    assert_eq!(state.pending_outputs[0].curves, pending);
}

#[test]
fn explicit_output_detach_appends_after_existing_pending_rows() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let attached = output("dev_output.csv", "attached/dev_output.csv");
    let plan = state.plan_output_imports(vec![attached]);
    state.commit_output_refinement(run_output_refinement(plan));
    let pending = output("other_output.csv", "pending/other_output.csv");
    state.add_pending_output(pending.source, pending.curves, PendingOutputReason::NoMatch);

    let plan = state
        .plan_output_clear(0, OutputRefinementPurpose::Detach)
        .expect("attached output plans a detach");
    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert!(report.action_succeeded);
    assert_eq!(state.selected_entry().unwrap().output_name(), None);
    assert_eq!(
        state
            .pending_outputs
            .iter()
            .map(PendingOutput::name)
            .collect::<Vec<_>>(),
        vec!["other_output.csv", "dev_output.csv"]
    );
}

#[test]
fn rejected_output_clear_commit_preserves_both_attachments_and_revision() {
    // Core lifecycle tests force the real `RetainedDiblNotApplied`
    // detachment. This pins the GUI half of that typed terminal result
    // without duplicating Level 62 forward-model math in the GUI crate.
    for purpose in [
        OutputRefinementPurpose::Detach,
        OutputRefinementPurpose::Remove,
    ] {
        let (mut state, _) = state_with_output_and_dibl();
        let revision = state.selected_entry().unwrap().revision();
        let output_name = state
            .selected_entry()
            .unwrap()
            .output_name()
            .unwrap()
            .to_owned();
        let dibl_name = state
            .selected_entry()
            .unwrap()
            .dibl_name()
            .unwrap()
            .to_owned();
        let plan = state
            .plan_output_clear(0, purpose)
            .expect("attached output plans a clear");
        let mut result = run_output_refinement(plan);
        let job = &mut result.jobs[0];
        job.changed = false;
        job.clear_pending = None;
        job.clear_error = Some(MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE.to_owned());

        let report = state.commit_output_refinement(result);

        assert!(!report.action_succeeded);
        let entry = state.selected_entry().unwrap();
        assert_eq!(entry.revision(), revision);
        assert_eq!(entry.output_name(), Some(output_name.as_str()));
        assert_eq!(entry.dibl_name(), Some(dibl_name.as_str()));
        assert!(state.pending_outputs.is_empty());
    }
}

#[test]
fn guarded_output_commit_follows_identity_across_index_reorder() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "first_transfer.csv", 1.0);
    add_device(&mut state, "second_transfer.csv", 2.0);
    let target_id = state.devices[1].id();
    let plan =
        state.plan_output_imports(vec![output("second_output.csv", "lot/second_output.csv")]);
    let result = run_output_refinement(plan);

    assert!(state.remove_device(0));
    let report = state.commit_output_refinement(result);
    assert_eq!(report.attached, 1);
    assert_eq!(report.unfittable, 1);
    let target = state
        .devices
        .iter()
        .find(|entry| entry.id() == target_id)
        .expect("target survives at a new index");
    assert_eq!(target.output_name(), Some("second_output.csv"));
}

#[test]
fn removed_output_target_becomes_device_changed_pending_data() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let plan = state.plan_output_imports(vec![output("dev_output.csv", "lot/dev_output.csv")]);
    let result = run_output_refinement(plan);
    assert!(state.remove_device(0));

    let report = state.commit_output_refinement(result);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].reason(),
        PendingOutputReason::DeviceChanged
    );
}

#[test]
fn scientific_edit_makes_output_result_stale_without_overwriting_live_state() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let plan = state.plan_output_imports(vec![output("dev_output.csv", "lot/dev_output.csv")]);
    let result = run_output_refinement(plan);
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.5, fit.gamma, fit.k)
        .unwrap();
    let edited_vt = state.selected_entry().unwrap().device().aostft_fit().vt;

    state.commit_output_refinement(result);
    assert_eq!(
        state.selected_entry().unwrap().device().aostft_fit().vt,
        edited_vt
    );
    assert_eq!(state.selected_entry().unwrap().output_name(), None);
    assert_eq!(
        state.pending_outputs[0].reason(),
        PendingOutputReason::DeviceChanged
    );
}

#[test]
fn same_device_output_batch_preserves_dialog_order_and_displaced_source() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let revision = state.selected_entry().unwrap().revision();
    let plan = state.plan_output_imports(vec![
        output("dev_output.csv", "lot-a/dev_output.csv"),
        output("dev_id-vd.csv", "lot-b/dev_id-vd.csv"),
    ]);
    assert_eq!(plan.jobs.len(), 1);
    let result = run_output_refinement(plan);
    let worker_science = &result.jobs[0].science;
    assert!(worker_science.device().has_output_curves());
    assert_eq!(
        worker_science.output_source().map(OutputSource::name),
        Some("dev_id-vd.csv")
    );
    let report = state.commit_output_refinement(result);
    assert_eq!(report.displaced, 1);

    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.output_name(), Some("dev_id-vd.csv"));
    assert_eq!(
        entry.output_source_path(),
        Some(std::path::Path::new("lot-b/dev_id-vd.csv"))
    );
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(state.pending_outputs[0].name(), "dev_output.csv");
    assert_eq!(
        state.pending_outputs[0].reason(),
        PendingOutputReason::Detached
    );
    assert_eq!(entry.revision().get(), revision.get() + 1);
}

#[test]
fn ordered_output_batch_keeps_the_last_attachment_and_every_displaced_source() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let initial = output("dev_output.csv", "lot-initial/dev_output.csv");
    let plan = state.plan_output_imports(vec![initial]);
    state.commit_output_refinement(run_output_refinement(plan));

    let plan = state.plan_output_imports(vec![
        output("dev_output.csv", "lot-first/dev_output.csv"),
        output("dev_id-vd.csv", "lot-second/dev_id-vd.csv"),
    ]);
    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert_eq!(report.displaced, 2);
    assert_eq!(
        state.selected_entry().unwrap().output_source_path(),
        Some(std::path::Path::new("lot-second/dev_id-vd.csv"))
    );
    assert_eq!(
        state
            .pending_outputs
            .iter()
            .map(|pending| pending.source.path().unwrap())
            .collect::<Vec<_>>(),
        vec![
            std::path::Path::new("lot-initial/dev_output.csv"),
            std::path::Path::new("lot-first/dev_output.csv"),
        ]
    );
    assert!(state
        .pending_outputs
        .iter()
        .all(|pending| pending.reason() == PendingOutputReason::Detached));
}

#[test]
fn precommand_output_displacements_fold_generation_ordered_match_sets() {
    let pending = |path: Option<&str>, marker: f64| PendingOutput {
        source: OutputSource::new("shared_output.csv", path.map(Into::into)).unwrap(),
        curves: vec![OutputCurve {
            vg: 4.0,
            vds: vec![0.0, 1.0],
            id: vec![0.0, marker],
        }],
        reason: PendingOutputReason::Detached,
    };
    let effect =
        |ordinal, pending| OutputPendingEffect::PreCommandDisplacement { ordinal, pending };

    let mut state = ModelFitState::default();
    let latest = pending(Some("lot/shared.csv"), 2.0e-6);
    apply_output_pending_effects(
        &mut state,
        vec![
            effect(2, latest.clone()),
            effect(1, pending(Some("lot/shared.csv"), 1.0e-6)),
        ],
    );
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(state.pending_outputs[0].curves, latest.curves);

    let mut state = ModelFitState::default();
    apply_output_pending_effects(
        &mut state,
        vec![
            effect(1, pending(Some("lot-a/shared.csv"), 1.0e-6)),
            effect(2, pending(Some("lot-b/shared.csv"), 2.0e-6)),
            effect(3, pending(None, 3.0e-6)),
        ],
    );
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(state.pending_outputs[0].source.path(), None);

    let mut state = ModelFitState::default();
    apply_output_pending_effects(
        &mut state,
        vec![
            effect(1, pending(None, 1.0e-6)),
            effect(2, pending(Some("lot-a/shared.csv"), 2.0e-6)),
            effect(3, pending(Some("lot-b/shared.csv"), 3.0e-6)),
        ],
    );
    assert_eq!(
        state
            .pending_outputs
            .iter()
            .map(|pending| pending.source.path().unwrap())
            .collect::<Vec<_>>(),
        vec![
            std::path::Path::new("lot-a/shared.csv"),
            std::path::Path::new("lot-b/shared.csv"),
        ]
    );
}

#[test]
fn cleared_pathless_alias_cannot_destroy_a_distinct_displaced_output() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let initial = output("dev_output.csv", "lot-a/dev_output.csv");
    let displaced_curves = initial.curves.clone();
    let plan = state.plan_output_imports(vec![initial]);
    state.commit_output_refinement(run_output_refinement(plan));
    state.add_pending_output(
        OutputSource::new("dev_output.csv", None).unwrap(),
        output("dev_output.csv", "pending/dev_output.csv").curves,
        PendingOutputReason::NoMatch,
    );

    let plan = state.plan_output_imports(vec![output("dev_output.csv", "lot-b/dev_output.csv")]);
    let report = state.commit_output_refinement(run_output_refinement(plan));

    assert_eq!(report.displaced, 1);
    assert_eq!(
        state.selected_entry().unwrap().output_source_path(),
        Some(std::path::Path::new("lot-b/dev_output.csv"))
    );
    assert_eq!(state.pending_outputs.len(), 1);
    assert_eq!(
        state.pending_outputs[0].source.path(),
        Some(std::path::Path::new("lot-a/dev_output.csv"))
    );
    assert_eq!(
        state.pending_outputs[0].reason(),
        PendingOutputReason::Detached
    );
    assert_eq!(state.pending_outputs[0].curves, displaced_curves);
}

#[test]
fn canonical_output_alias_reload_clears_pending_without_detaching_the_source() {
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let direct_path = crate_root.join("Cargo.toml");
    let alias_path = crate_root.join("src").join("..").join("Cargo.toml");
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);

    let initial = output_at("dev_output.csv", direct_path);
    let initial_curves = initial.curves.clone();
    let plan = state.plan_output_imports(vec![initial]);
    let report = state.commit_output_refinement(run_output_refinement(plan));
    assert_eq!(report.displaced, 0);
    assert!(state.pending_outputs.is_empty());

    state.add_pending_output(
        OutputSource::new("dev_output_stale.csv", Some(alias_path.clone())).unwrap(),
        initial_curves,
        PendingOutputReason::NoMatch,
    );
    let reload = output_at("dev_id-vd.csv", alias_path.clone());
    let plan = state.plan_output_imports(vec![reload]);
    let report = state.commit_output_refinement(run_output_refinement(plan));
    assert_eq!(report.displaced, 0);

    let entry = state.selected_entry().expect("device remains selected");
    assert_eq!(entry.output_name(), Some("dev_id-vd.csv"));
    assert_eq!(entry.output_source_path(), Some(alias_path.as_path()));
    assert!(state.pending_outputs.is_empty());
}

#[test]
fn output_detach_result_is_valid_before_one_atomic_commit() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let plan = state.plan_output_imports(vec![output("dev_output.csv", "lot/dev_output.csv")]);
    state.commit_output_refinement(run_output_refinement(plan));
    let revision = state.selected_entry().unwrap().revision();
    let plan = state
        .plan_output_clear(0, OutputRefinementPurpose::Detach)
        .expect("attached output plans a detach");
    let result = run_output_refinement(plan);
    let worker_science = &result.jobs[0].science;
    assert!(!worker_science.device().has_output_curves());
    assert!(worker_science.output_source().is_none());

    let report = state.commit_output_refinement(result);
    let entry = state.selected_entry().unwrap();
    assert!(!entry.device().has_output_curves());
    assert_eq!(entry.output_name(), None);
    assert_eq!(entry.revision().get(), revision.get() + 1);
    assert_eq!(state.pending_outputs().len(), 1);
    assert_eq!(state.pending_outputs()[0].name(), "dev_output.csv");
    assert!(report.action_succeeded);
}
