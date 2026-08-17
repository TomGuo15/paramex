use super::super::add_test_device as add_device;
use super::*;
use crate::workspaces::modelfit::state::synthetic_transfer;
use paramex_core::modelfit::{FitModel, ModelParams};

fn fitting_second(vt: f64) -> SecondTransfer {
    let params = ModelParams {
        vt,
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

fn no_fit_second(primary_vds: f64, vt: f64) -> SecondTransfer {
    let mut second = fitting_second(vt);
    second.v_ds = primary_vds;
    second
}

#[test]
fn panic_recovery_keeps_every_dibl_input_and_parse_issue_without_mutating_science() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let revision = state.selected_entry().unwrap().revision();
    let names = [
        "Id-Vg-low [(1) ; first].csv",
        "Id-Vg-VD2V [(1) ; second].csv",
    ];
    let plan = state.plan_dibl_refinement(
        names
            .iter()
            .map(|name| DiblImport {
                source: DiblSource::new(*name, None).unwrap(),
                second: fitting_second(1.5),
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

    let report = state.recover_dibl_refinement(plan.panic_recovery());

    assert_eq!(report.purpose, DiblRefinementPurpose::Import);
    assert_eq!(report.mode, DiblRefinementMode::Batch);
    assert_eq!(report.recovered, 2);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(state.selected_entry().unwrap().revision(), revision);
    assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
    assert_eq!(
        state
            .pending_dibls()
            .iter()
            .map(|pending| (pending.name(), pending.reason()))
            .collect::<Vec<_>>(),
        vec![
            (names[0], PendingDiblReason::WorkerFailed),
            (names[1], PendingDiblReason::WorkerFailed),
        ]
    );
}

#[test]
fn panic_recovery_updates_pending_dibl_reason_without_attaching() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "device.csv", 1.0);
    state.add_pending_dibl(
        DiblSource::new("device-low.csv", None).unwrap(),
        fitting_second(1.5),
        PendingDiblReason::NoMatch,
    );
    let revision = state.selected_entry().unwrap().revision();
    let plan = state
        .plan_pending_dibl_attach(0)
        .expect("selected device and pending DIBL");

    let report = state.recover_dibl_refinement(plan.panic_recovery());

    assert_eq!(report.purpose, DiblRefinementPurpose::AttachPending);
    assert_eq!(report.mode, DiblRefinementMode::Single);
    assert_eq!(report.recovered, 1);
    assert_eq!(state.selected_entry().unwrap().revision(), revision);
    assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
    assert_eq!(
        state.pending_dibls()[0].reason(),
        PendingDiblReason::WorkerFailed
    );
}

#[test]
fn panic_recovery_counts_the_generation_ordered_dibl_match_set() {
    fn import(path: Option<&str>, marker: f64) -> DiblImport {
        let mut second = fitting_second(1.5);
        second.v_ds = marker;
        DiblImport {
            source: DiblSource::new("shared_dibl.csv", path.map(std::path::PathBuf::from)).unwrap(),
            second,
        }
    }

    fn recover(imports: Vec<DiblImport>) -> (ModelFitState, usize) {
        let mut state = ModelFitState::default();
        let recovery = DiblRefinementRecovery {
            purpose: DiblRefinementPurpose::Import,
            mode: DiblRefinementMode::Batch,
            pending: imports
                .into_iter()
                .enumerate()
                .map(|(ordinal, import)| DeferredDibl {
                    ordinal,
                    import,
                    reason: PendingDiblReason::WorkerFailed,
                })
                .collect(),
            issues: Vec::new(),
        };
        let recovered = state.recover_dibl_refinement(recovery).recovered;
        (state, recovered)
    }

    let (latest_pathless, recovered) = recover(vec![
        import(Some("lot-a/shared_dibl.csv"), 1.0),
        import(Some("lot-b/shared_dibl.csv"), 2.0),
        import(None, 3.0),
    ]);
    assert_eq!(recovered, 1);
    assert_eq!(latest_pathless.pending_dibls.len(), 1);
    assert_eq!(latest_pathless.pending_dibls[0].source.path(), None);
    assert_eq!(latest_pathless.pending_dibls[0].second.v_ds, 3.0);

    let (latest_pathful, recovered) = recover(vec![
        import(None, 3.0),
        import(Some("lot-a/shared_dibl.csv"), 1.0),
        import(Some("lot-b/shared_dibl.csv"), 2.0),
    ]);
    assert_eq!(recovered, 2);
    assert_eq!(
        latest_pathful
            .pending_dibls
            .iter()
            .map(|pending| pending.source.path())
            .collect::<Vec<_>>(),
        vec![
            Some(std::path::Path::new("lot-a/shared_dibl.csv")),
            Some(std::path::Path::new("lot-b/shared_dibl.csv")),
        ]
    );
}

#[test]
fn panic_recovery_for_dibl_clear_purposes_leaves_attachment_unchanged() {
    for purpose in [DiblRefinementPurpose::Detach, DiblRefinementPurpose::Remove] {
        let mut state = ModelFitState::default();
        add_device(&mut state, "device.csv", 1.0);
        let plan = state.plan_dibl_refinement(
            vec![DiblImport {
                source: DiblSource::new("device-low.csv", None).unwrap(),
                second: fitting_second(1.5),
            }],
            state.selected_device_id(),
            true,
            Vec::new(),
        );
        let report = state.commit_dibl_refinement(run_dibl_refinement(plan));
        assert!(report.action_succeeded);
        let revision = state.selected_entry().unwrap().revision();
        let plan = state
            .plan_dibl_clear(0, purpose)
            .expect("attached DIBL plans a clear");

        let report = state.recover_dibl_refinement(plan.panic_recovery());

        assert_eq!(report.purpose, purpose);
        assert_eq!(report.recovered, 0);
        assert_eq!(state.selected_entry().unwrap().revision(), revision);
        assert_eq!(
            state.selected_entry().unwrap().dibl_name(),
            Some("device-low.csv")
        );
        assert!(state.pending_dibls().is_empty());
    }
}

#[test]
fn single_dibl_plan_keeps_the_click_time_target_after_selection_changes() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "first_transfer.csv", 1.0);
    add_device(&mut state, "second_transfer.csv", 2.0);
    let captured = state.selected_device_id().unwrap();
    state.select(1);
    let plan = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new("picked.csv", None).unwrap(),
            second: SecondTransfer {
                vg: vec![0.0; 12],
                id_abs: vec![1.0e-9; 12],
                v_ds: 1.0,
            },
        }],
        Some(captured),
        true,
        Vec::new(),
    );

    assert_eq!(plan.jobs.len(), 1);
    assert_eq!(plan.jobs[0].token.id, captured);
}

#[test]
fn removed_click_time_target_keeps_the_import_pending_and_counts_stale() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "device.csv", 1.0);
    let captured = state.selected_device_id();
    assert!(state.remove_device(0));
    let plan = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new("device-low.csv", None).unwrap(),
            second: fitting_second(1.5),
        }],
        captured,
        true,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));
    assert_eq!(report.stale, 1);
    assert!(report.fitted.is_empty());
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::DeviceChanged
    );
}

#[test]
fn same_device_dibl_inputs_stay_in_dialog_order_on_one_clone() {
    let mut state = ModelFitState::default();
    add_device(
        &mut state,
        "Id-Vg-high [(1) ; 5_1_2024 4_13_47 PM].csv",
        1.0,
    );
    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let second = || SecondTransfer {
        vg: vec![0.0; 12],
        id_abs: vec![1.0e-9; 12],
        v_ds: primary_vds,
    };
    let names = [
        "Id-Vg-low [(1) ; first].csv",
        "Id-Vg-VD2V [(1) ; second].csv",
    ];
    let plan = state.plan_dibl_refinement(
        names
            .iter()
            .map(|name| DiblImport {
                source: DiblSource::new(*name, None).unwrap(),
                second: second(),
            })
            .collect(),
        None,
        false,
        Vec::new(),
    );
    assert_eq!(plan.jobs.len(), 1);
    assert_eq!(
        plan.jobs[0]
            .operations
            .iter()
            .map(|operation| match operation {
                DiblOperation::Replace { import, .. } => import.source.name.as_str(),
                DiblOperation::Clear { .. } => panic!("import plan contains a clear"),
            })
            .collect::<Vec<_>>(),
        names
    );
    let result = run_dibl_refinement(plan);
    assert_eq!(
        result.jobs[0]
            .operations
            .iter()
            .map(|operation| match operation {
                DiblOperationResult::Fitted { import, .. }
                | DiblOperationResult::NoFit { import, .. } => {
                    import.source.name.as_str()
                }
            })
            .collect::<Vec<_>>(),
        names
    );
    let report = state.commit_dibl_refinement(result);
    assert!(report.fitted.is_empty());
    assert_eq!(report.issues.len(), 2);
    assert_eq!(
        state
            .pending_dibls
            .iter()
            .map(|pending| (pending.name(), pending.reason()))
            .collect::<Vec<_>>(),
        names
            .iter()
            .map(|name| (*name, PendingDiblReason::NoFit))
            .collect::<Vec<_>>()
    );
}

#[test]
fn unmatched_and_ambiguous_dibl_imports_are_recoverable_pending_data() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(2) ; primary].csv", 1.0);
    add_device(&mut state, "Id-Vg-low [(2) ; primary].csv", 2.0);
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new("missing-low.csv", Some("lot/missing.csv".into())).unwrap(),
                second: fitting_second(1.5),
            },
            DiblImport {
                source: DiblSource::new(
                    "Id-Vg-VD2V [(2) ; candidate].csv",
                    Some("lot/candidate.csv".into()),
                )
                .unwrap(),
                second: fitting_second(1.5),
            },
        ],
        None,
        false,
        Vec::new(),
    );

    assert!(plan.jobs.is_empty());
    assert_eq!(plan.summary.unmatched, 1);
    assert_eq!(plan.summary.ambiguous, 1);
    assert!(state.pending_dibls.is_empty());
    state.commit_dibl_refinement(run_dibl_refinement(plan));
    assert_eq!(
        state
            .pending_dibls
            .iter()
            .map(|pending| (pending.name(), pending.reason()))
            .collect::<Vec<_>>(),
        vec![
            ("missing-low.csv", PendingDiblReason::NoMatch),
            (
                "Id-Vg-VD2V [(2) ; candidate].csv",
                PendingDiblReason::Ambiguous
            ),
        ]
    );
}

#[test]
fn new_same_source_no_fit_beats_the_detached_pre_job_attachment() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let same_source = "Id-Vg-low [(1) ; same-source].csv";
    let attach = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new(same_source, None).unwrap(),
            second: fitting_second(1.5),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    assert!(
        state
            .commit_dibl_refinement(run_dibl_refinement(attach))
            .action_succeeded
    );

    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let newer = no_fit_second(primary_vds, 1.75);
    let newer_id = newer.id_abs.clone();
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new(same_source, None).unwrap(),
                second: newer,
            },
            DiblImport {
                source: DiblSource::new("Id-Vg-VD2V [(1) ; replacement].csv", None).unwrap(),
                second: fitting_second(2.0),
            },
        ],
        None,
        false,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.fitted.len(), 1);
    assert_eq!(report.displaced, 1);
    assert_eq!(state.pending_dibls.len(), 1);
    let pending = &state.pending_dibls[0];
    assert_eq!(pending.name(), same_source);
    assert_eq!(pending.reason(), PendingDiblReason::NoFit);
    assert_eq!(pending.second.v_ds, primary_vds);
    assert_eq!(pending.second.id_abs, newer_id);
}

#[test]
fn later_same_source_success_suppresses_an_earlier_no_fit() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let source = "Id-Vg-low [(1) ; reload].csv";
    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new(source, None).unwrap(),
                second: no_fit_second(primary_vds, 1.5),
            },
            DiblImport {
                source: DiblSource::new(source, None).unwrap(),
                second: fitting_second(1.75),
            },
        ],
        None,
        false,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.fitted.len(), 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].kind, DiblIssueKind::NoFit);
    assert_eq!(state.selected_entry().unwrap().dibl_name(), Some(source));
    assert!(state.pending_dibls.is_empty());
}

#[test]
fn stale_same_source_success_does_not_suppress_the_latest_import() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let source = "Id-Vg-low [(1) ; reload].csv";
    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new(source, None).unwrap(),
                second: no_fit_second(primary_vds, 1.5),
            },
            DiblImport {
                source: DiblSource::new(source, None).unwrap(),
                second: fitting_second(1.75),
            },
        ],
        None,
        false,
        Vec::new(),
    );
    let result = run_dibl_refinement(plan);
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.25, fit.gamma, fit.k)
        .unwrap();

    let report = state.commit_dibl_refinement(result);

    assert_eq!(report.stale, 2);
    assert!(report.fitted.is_empty());
    assert_eq!(state.selected_entry().unwrap().dibl_name(), None);
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].name(), source);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::DeviceChanged
    );
    assert_eq!(state.pending_dibls[0].second.v_ds, 1.0);
}

#[test]
fn deferred_unmatched_and_ambiguous_imports_keep_global_order_on_commit_and_recovery() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 0.75);
    add_device(&mut state, "Id-Vg-high [(2) ; primary].csv", 1.0);
    add_device(&mut state, "Id-Vg-low [(2) ; primary].csv", 2.0);
    let shared_path = std::path::PathBuf::from("lot/shared-dibl.csv");
    let matched = || DiblImport {
        source: DiblSource::new("Id-Vg-low [(1) ; candidate].csv", Some(shared_path.clone()))
            .unwrap(),
        second: fitting_second(1.25),
    };
    let unmatched = || DiblImport {
        source: DiblSource::new("missing-low.csv", Some(shared_path.clone())).unwrap(),
        second: fitting_second(1.5),
    };
    let ambiguous = || DiblImport {
        source: DiblSource::new(
            "Id-Vg-VD2V [(2) ; candidate].csv",
            Some(shared_path.clone()),
        )
        .unwrap(),
        second: fitting_second(1.75),
    };

    let plan = state.plan_dibl_refinement(vec![matched(), unmatched()], None, false, Vec::new());
    assert!(state.pending_dibls.is_empty());
    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));
    assert_eq!(report.fitted.len(), 1);
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].name(), "missing-low.csv");
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::NoMatch);

    assert!(state.remove_pending_dibl(0));
    let plan = state.plan_dibl_refinement(vec![matched(), ambiguous()], None, false, Vec::new());
    state.commit_dibl_refinement(run_dibl_refinement(plan));
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::Ambiguous
    );

    assert!(state.remove_pending_dibl(0));
    let plan = state.plan_dibl_refinement(vec![unmatched(), matched()], None, false, Vec::new());
    state.commit_dibl_refinement(run_dibl_refinement(plan));
    assert!(state.pending_dibls.is_empty());

    let plan = state.plan_dibl_refinement(vec![matched(), ambiguous()], None, false, Vec::new());
    let report = state.recover_dibl_refinement(plan.panic_recovery());
    assert_eq!(report.recovered, 1);
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].name(),
        "Id-Vg-VD2V [(2) ; candidate].csv"
    );
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::Ambiguous
    );

    assert!(state.remove_pending_dibl(0));
    let plan = state.plan_dibl_refinement(vec![unmatched(), matched()], None, false, Vec::new());
    state.recover_dibl_refinement(plan.panic_recovery());
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::WorkerFailed
    );
}

#[test]
fn pre_existing_pending_beats_detaching_an_older_same_source_attachment() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "device.csv", 1.0);
    let source = DiblSource::new("device-low.csv", None).unwrap();
    let attach = state.plan_dibl_refinement(
        vec![DiblImport {
            source: source.clone(),
            second: fitting_second(1.5),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    state.commit_dibl_refinement(run_dibl_refinement(attach));
    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let newer = no_fit_second(primary_vds, 1.75);
    let newer_id = newer.id_abs.clone();
    state.add_pending_dibl(source, newer, PendingDiblReason::NoFit);

    let detach = state
        .plan_dibl_clear(0, DiblRefinementPurpose::Detach)
        .expect("attached DIBL plans a detach");
    let report = state.commit_dibl_refinement(run_dibl_refinement(detach));

    assert!(report.action_succeeded);
    assert!(state.selected_entry().unwrap().dibl_name().is_none());
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::NoFit);
    assert_eq!(state.pending_dibls[0].second.id_abs, newer_id);
}

#[test]
fn latest_fresh_same_source_payload_wins_across_grouped_jobs() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    add_device(&mut state, "Id-Vg-high [(2) ; primary].csv", 2.0);
    let revisions = state
        .devices
        .iter()
        .map(|entry| entry.revision())
        .collect::<Vec<_>>();
    let shared_path = std::path::PathBuf::from("lot/grouped-dibl.csv");
    let first_vds = state.devices[0].device().bias().v_ds;
    let second_vds = state.devices[1].device().bias().v_ds;
    let newest = no_fit_second(first_vds, 2.0);
    let newest_id = newest.id_abs.clone();
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new("Id-Vg-low [(1) ; first].csv", Some(shared_path.clone()))
                    .unwrap(),
                second: no_fit_second(first_vds, 1.25),
            },
            DiblImport {
                source: DiblSource::new("Id-Vg-low [(2) ; middle].csv", Some(shared_path.clone()))
                    .unwrap(),
                second: no_fit_second(second_vds, 1.5),
            },
            DiblImport {
                source: DiblSource::new("Id-Vg-VD2V [(1) ; newest].csv", Some(shared_path))
                    .unwrap(),
                second: newest,
            },
        ],
        None,
        false,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.issues.len(), 3);
    assert!(!report.action_succeeded);
    assert_eq!(
        state
            .devices
            .iter()
            .map(|entry| entry.revision())
            .collect::<Vec<_>>(),
        revisions
    );
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].name(),
        "Id-Vg-VD2V [(1) ; newest].csv"
    );
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::NoFit);
    assert_eq!(state.pending_dibls[0].second.id_abs, newest_id);
}

#[test]
fn displaced_fresh_fit_keeps_its_original_generation() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let shared_path = std::path::PathBuf::from("lot/original-generation.csv");
    let mut deferred = fitting_second(1.6);
    deferred.v_ds = 0.37;
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new(
                    "Id-Vg-low [(1) ; accepted].csv",
                    Some(shared_path.clone()),
                )
                .unwrap(),
                second: fitting_second(1.5),
            },
            DiblImport {
                source: DiblSource::new("missing-low.csv", Some(shared_path)).unwrap(),
                second: deferred,
            },
            DiblImport {
                source: DiblSource::new(
                    "Id-Vg-VD2V [(1) ; replacement].csv",
                    Some("lot/replacement.csv".into()),
                )
                .unwrap(),
                second: fitting_second(1.75),
            },
        ],
        None,
        false,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.fitted.len(), 2);
    assert_eq!(report.unmatched, 1);
    assert_eq!(report.displaced, 1);
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].name(), "missing-low.csv");
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::NoMatch);
    assert_eq!(state.pending_dibls[0].second.v_ds, 0.37);
}

#[test]
fn ordered_dibl_batch_keeps_the_last_attachment_and_every_displaced_source() {
    let mut state = ModelFitState::default();
    let primary_name = "Id-Vg-high [(1) ; primary].csv";
    add_device(&mut state, primary_name, 1.0);
    let target = state.selected_device_id();
    let initial = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new("Id-Vg-low [(1) ; initial].csv", None).unwrap(),
            second: fitting_second(1.5),
        }],
        target,
        true,
        Vec::new(),
    );
    let initial_report = state.commit_dibl_refinement(run_dibl_refinement(initial));
    assert_eq!(initial_report.fitted.len(), 1);

    let names = [
        "Id-Vg-low [(1) ; first].csv",
        "Id-Vg-VD2V [(1) ; second].csv",
    ];
    let plan = state.plan_dibl_refinement(
        names
            .iter()
            .map(|name| DiblImport {
                source: DiblSource::new(*name, None).unwrap(),
                second: fitting_second(1.5),
            })
            .collect(),
        None,
        false,
        Vec::new(),
    );
    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.fitted.len(), 2);
    assert_eq!(report.displaced, 2);
    assert_eq!(state.selected_entry().unwrap().dibl_name(), Some(names[1]));
    assert_eq!(
        state
            .pending_dibls
            .iter()
            .map(PendingDibl::name)
            .collect::<Vec<_>>(),
        vec!["Id-Vg-low [(1) ; initial].csv", names[0]]
    );
    assert!(state
        .pending_dibls
        .iter()
        .all(|pending| pending.reason() == PendingDiblReason::Detached));
}

#[test]
fn precommand_dibl_displacements_fold_generation_ordered_match_sets() {
    let pending = |path: Option<&str>, vt: f64| PendingDibl {
        source: DiblSource::new("shared-low.csv", path.map(Into::into)).unwrap(),
        second: fitting_second(vt),
        reason: PendingDiblReason::Detached,
    };
    let effect = |ordinal, pending| DiblPendingEffect::PreCommandDisplacement { ordinal, pending };

    let mut state = ModelFitState::default();
    let latest = pending(Some("lot/shared.csv"), 1.75);
    apply_dibl_pending_effects(
        &mut state,
        vec![
            effect(2, latest.clone()),
            effect(1, pending(Some("lot/shared.csv"), 1.5)),
        ],
    );
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].second, latest.second);

    let mut state = ModelFitState::default();
    apply_dibl_pending_effects(
        &mut state,
        vec![
            effect(1, pending(Some("lot-a/shared.csv"), 1.5)),
            effect(2, pending(Some("lot-b/shared.csv"), 1.6)),
            effect(3, pending(None, 1.7)),
        ],
    );
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].source.path(), None);

    let mut state = ModelFitState::default();
    apply_dibl_pending_effects(
        &mut state,
        vec![
            effect(1, pending(None, 1.5)),
            effect(2, pending(Some("lot-a/shared.csv"), 1.6)),
            effect(3, pending(Some("lot-b/shared.csv"), 1.7)),
        ],
    );
    assert_eq!(
        state
            .pending_dibls
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
fn cleared_pathless_alias_cannot_destroy_a_distinct_displaced_dibl() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let name = "Id-Vg-low [(1) ; shared].csv";
    let displaced_second = fitting_second(1.5);
    let attach = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new(name, Some("lot-a/shared.csv".into())).unwrap(),
            second: displaced_second.clone(),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    state.commit_dibl_refinement(run_dibl_refinement(attach));
    state.add_pending_dibl(
        DiblSource::new(name, None).unwrap(),
        fitting_second(1.6),
        PendingDiblReason::NoMatch,
    );

    let replace = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new(name, Some("lot-b/shared.csv".into())).unwrap(),
            second: fitting_second(1.75),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    let report = state.commit_dibl_refinement(run_dibl_refinement(replace));

    assert_eq!(report.displaced, 1);
    assert_eq!(
        state.selected_entry().unwrap().dibl_source_path(),
        Some(std::path::Path::new("lot-b/shared.csv"))
    );
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].source.path(),
        Some(std::path::Path::new("lot-a/shared.csv"))
    );
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::Detached);
    assert_eq!(state.pending_dibls[0].second, displaced_second);
}

#[test]
fn mixed_success_and_no_fit_commits_once_and_keeps_the_failed_input_pending() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "Id-Vg-high [(1) ; primary].csv", 1.0);
    let revision = state.selected_entry().unwrap().revision().get();
    let primary_vds = state.selected_entry().unwrap().device().bias().v_ds;
    let successful_name = "Id-Vg-low [(1) ; success].csv";
    let failed_name = "Id-Vg-VD2V [(1) ; no-fit].csv";
    let mut no_fit = fitting_second(1.75);
    no_fit.v_ds = primary_vds;
    let plan = state.plan_dibl_refinement(
        vec![
            DiblImport {
                source: DiblSource::new(successful_name, None).unwrap(),
                second: fitting_second(1.5),
            },
            DiblImport {
                source: DiblSource::new(failed_name, None).unwrap(),
                second: no_fit,
            },
        ],
        None,
        false,
        Vec::new(),
    );

    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert!(report.action_succeeded);
    assert_eq!(report.fitted.len(), 1);
    assert_eq!(report.fitted[0].name, successful_name);
    assert_eq!(report.fitted[0].second_vds, 1.0);
    assert!(report.fitted[0].at.is_finite());
    assert_eq!(report.displaced, 0);
    assert_eq!(report.unmatched, 0);
    assert_eq!(report.ambiguous, 0);
    assert_eq!(report.stale, 0);
    assert_eq!(report.commit_errors, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].name, failed_name);
    assert_eq!(report.issues[0].kind, DiblIssueKind::NoFit);
    assert!(!report.issues[0].message.is_empty());
    let entry = state.selected_entry().unwrap();
    assert_eq!(entry.revision().get(), revision + 1);
    assert_eq!(entry.dibl_name(), Some(successful_name));
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(state.pending_dibls[0].name(), failed_name);
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::NoFit);
}

#[test]
fn attached_dibl_can_detach_to_pending_and_reattach() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "device.csv", 1.0);
    let attach = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new("device-low.csv", None).unwrap(),
            second: fitting_second(1.5),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    state.commit_dibl_refinement(run_dibl_refinement(attach));
    assert_eq!(
        state.selected_entry().unwrap().dibl_name(),
        Some("device-low.csv")
    );

    let detach = state
        .plan_dibl_clear(0, DiblRefinementPurpose::Detach)
        .expect("attached DIBL plans a detach");
    let report = state.commit_dibl_refinement(run_dibl_refinement(detach));
    assert!(report.action_succeeded);
    assert!(state.selected_entry().unwrap().dibl_name().is_none());
    assert_eq!(state.pending_dibls[0].reason(), PendingDiblReason::Detached);

    let attach = state
        .plan_pending_dibl_attach(0)
        .expect("pending DIBL attaches to selected");
    let report = state.commit_dibl_refinement(run_dibl_refinement(attach));
    assert!(report.action_succeeded);
    assert_eq!(
        state.selected_entry().unwrap().dibl_name(),
        Some("device-low.csv")
    );
    assert!(state.pending_dibls.is_empty());
}

#[test]
fn successful_same_source_attach_clears_a_canonical_pending_alias() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let alias = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("..")
        .join("Cargo.toml");
    let mut state = ModelFitState::default();
    add_device(&mut state, "device.csv", 1.0);
    state.add_pending_dibl(
        DiblSource::new("pending-label.csv", Some(alias)).unwrap(),
        fitting_second(1.5),
        PendingDiblReason::NoMatch,
    );

    let plan = state.plan_dibl_refinement(
        vec![DiblImport {
            source: DiblSource::new("attached-label.csv", Some(manifest)).unwrap(),
            second: fitting_second(1.5),
        }],
        state.selected_device_id(),
        true,
        Vec::new(),
    );
    let report = state.commit_dibl_refinement(run_dibl_refinement(plan));

    assert_eq!(report.fitted.len(), 1);
    assert_eq!(report.displaced, 0);
    assert!(state.pending_dibls.is_empty());
}

#[test]
fn pending_dibl_upsert_uses_canonical_and_pathless_source_identity() {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let alias = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("..")
        .join("Cargo.toml");
    let mut state = ModelFitState::default();
    let mut first = fitting_second(1.5);
    first.v_ds = 0.25;
    state.add_pending_dibl(
        DiblSource::new("first-label.csv", Some(alias)).unwrap(),
        first,
        PendingDiblReason::NoMatch,
    );
    let mut canonical_reload = fitting_second(1.75);
    canonical_reload.v_ds = 0.5;
    state.add_pending_dibl(
        DiblSource::new("current-label.csv", Some(manifest)).unwrap(),
        canonical_reload,
        PendingDiblReason::Ambiguous,
    );
    let mut pathless_reload = fitting_second(2.0);
    pathless_reload.v_ds = 0.75;
    state.add_pending_dibl(
        DiblSource::new("current-label.csv", None).unwrap(),
        pathless_reload,
        PendingDiblReason::NoFit,
    );

    assert_eq!(state.pending_dibls.len(), 1);
    let pending = &state.pending_dibls[0];
    assert_eq!(pending.name(), "current-label.csv");
    assert_eq!(pending.source_path(), None);
    assert_eq!(pending.reason(), PendingDiblReason::NoFit);
    assert_eq!(pending.second.v_ds, 0.75);
}

#[test]
fn stale_dibl_commit_does_not_replace_the_edited_live_device() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let entry = &state.devices[0];
    let token = entry.token();
    let worker_science = entry.science().clone();
    let result = DiblRefinementResult {
        purpose: DiblRefinementPurpose::Import,
        mode: DiblRefinementMode::Single,
        jobs: vec![DiblJobResult {
            token,
            device_name: "dev_transfer.csv".into(),
            science: worker_science,
            operations: vec![DiblOperationResult::Fitted {
                ordinal: 0,
                import: DiblImport {
                    source: DiblSource::new("dev_low.csv", None).unwrap(),
                    second: SecondTransfer {
                        vg: vec![0.0; 12],
                        id_abs: vec![1.0e-9; 12],
                        v_ds: 1.0,
                    },
                },
                at: 1.0e-8,
                displaced: None,
            }],
            clear_pending: None,
            changed: true,
            clear_error: None,
        }],
        summary: DiblImportSummary::default(),
    };
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.75, fit.gamma, fit.k)
        .unwrap();
    let edited_vt = state.selected_entry().unwrap().device().aostft_fit().vt;

    let report = state.commit_dibl_refinement(result);
    assert!(report.fitted.is_empty());
    assert_eq!(report.issues[0].kind, DiblIssueKind::Stale);
    assert_eq!(state.pending_dibls.len(), 1);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::DeviceChanged
    );
    assert_eq!(
        state.selected_entry().unwrap().device().aostft_fit().vt,
        edited_vt
    );
    assert!(state
        .selected_entry()
        .unwrap()
        .device()
        .model(FitModel::Aostft)
        .is_manual());
}

#[test]
fn stale_dibl_commit_discards_obsolete_no_fit_diagnostics() {
    let mut state = ModelFitState::default();
    add_device(&mut state, "dev_transfer.csv", 1.0);
    let entry = &state.devices[0];
    let result = DiblRefinementResult {
        purpose: DiblRefinementPurpose::Import,
        mode: DiblRefinementMode::Single,
        jobs: vec![DiblJobResult {
            token: entry.token(),
            device_name: "dev_transfer.csv".into(),
            science: entry.science().clone(),
            operations: vec![DiblOperationResult::NoFit {
                ordinal: 0,
                import: DiblImport {
                    source: DiblSource::new("dev_low.csv", None).unwrap(),
                    second: SecondTransfer {
                        vg: vec![0.0; 12],
                        id_abs: vec![1.0e-9; 12],
                        v_ds: 1.0,
                    },
                },
                message: "obsolete worker conclusion".to_owned(),
            }],
            clear_pending: None,
            changed: false,
            clear_error: None,
        }],
        summary: DiblImportSummary::default(),
    };
    let fit = *state.selected_entry().unwrap().device().aostft_fit();
    state
        .set_selected_fit(fit.vt + 0.75, fit.gamma, fit.k)
        .unwrap();

    let report = state.commit_dibl_refinement(result);

    assert_eq!(report.issues.len(), 1);
    assert_eq!(report.issues[0].kind, DiblIssueKind::Stale);
    assert_eq!(report.issues[0].message, MODEL_DIBL_STALE_MESSAGE);
    assert_eq!(
        state.pending_dibls[0].reason(),
        PendingDiblReason::DeviceChanged
    );
}
