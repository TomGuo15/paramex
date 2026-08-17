// Regression tests at the fitted-device lifecycle interface. These cover the
// orchestration that used to be duplicated by the GUI workspace state.

use super::{clone_fixture, FixtureCache};
use crate::modelfit::forward::{output_curve, transfer_curve};
use crate::modelfit::level62::{level62_output, level62_transfer};
use crate::modelfit::{
    AboveThresholdFit, DetachDiblError, DetachOutputError, DiblError, DiblReplacementError,
    EditError, FitDeviceError, FitModel, FittedDevice, GeometryParams, InputError, Level62Params,
    ModelParams, OutputAttachOutcome, OutputCurve, OutputParams, OutputReplacementError,
    RefitError, SecondTransfer,
};

const TNOM_K: f64 = 298.15;

fn aostft_device() -> FittedDevice {
    static FIXTURE: FixtureCache<FittedDevice> = FixtureCache::new();

    clone_fixture(&FIXTURE, || {
        let params = ModelParams {
            vt: 2.0,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg: Vec<_> = (0..=160).map(|i| -3.0 + 0.1 * i as f64).collect();
        let id = transfer_curve(&params, &vg);
        FittedDevice::fit("aostft".into(), vg, id).expect("synthetic transfer fits")
    })
}

fn output_family(vt: f64) -> Vec<OutputCurve> {
    output_family_with_params(
        vt,
        OutputParams {
            alpha_sat: 0.7,
            lambda: 0.01,
            m: 2.5,
        },
    )
}

fn output_family_with_params(vt: f64, params: OutputParams) -> Vec<OutputCurve> {
    let vds: Vec<_> = (0..=150).map(|i| i as f64 * 0.1).collect();
    [2.0, 4.0, 6.0, 8.0]
        .into_iter()
        .map(|overdrive| OutputCurve {
            vg: vt + overdrive,
            id: output_curve(vt, &params, 1.0e-5, vt + overdrive, &vds),
            vds: vds.clone(),
        })
        .collect()
}

fn recover_h_fit(strict: AboveThresholdFit, output: OutputParams, bias_v_ds: f64) -> (f64, f64) {
    (
        strict.gamma + 1.0,
        strict.k * output.alpha_sat * (1.0 + output.lambda * bias_v_ds) / bias_v_ds,
    )
}

fn assert_close(actual: f64, expected: f64, relative: f64) {
    let scale = expected.abs().max(f64::MIN_POSITIVE);
    assert!(
        (actual - expected).abs() <= relative * scale,
        "actual={actual:e}, expected={expected:e}"
    );
}

fn level62_measurements() -> (FittedDevice, Vec<OutputCurve>, SecondTransfer) {
    static FIXTURE: FixtureCache<(FittedDevice, Vec<OutputCurve>, SecondTransfer)> =
        FixtureCache::new();

    clone_fixture(&FIXTURE, || {
        let geometry = GeometryParams {
            w_um: 100.0,
            l_um: 10.0,
        };
        let truth = Level62Params {
            vto: 1.0,
            at: 3.0e-8,
            asat: 0.7,
            lambda: 0.025,
            vkink: 2.8,
            eta: 5.0,
            mu0: 80.0e-4,
            mu1: 0.03e-4,
            mmu: 2.2,
            mus: 3.0e-4,
            ..Level62Params::ltps()
        };
        let vg: Vec<_> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
        let primary = level62_transfer(&truth, geometry, TNOM_K, &vg, 8.0);
        let mut device = FittedDevice::fit("ltps".into(), vg.clone(), primary).expect("fits");
        device.set_geometry(geometry).expect("geometry commits");
        let vds: Vec<_> = (0..=80).map(|i| i as f64 * 0.15).collect();
        let output = [2.4, 3.2, 4.0, 4.8]
            .into_iter()
            .map(|gate| OutputCurve {
                vg: gate,
                id: level62_output(&truth, geometry, TNOM_K, gate, &vds),
                vds: vds.clone(),
            })
            .collect();
        let second = SecondTransfer {
            vg: vg.clone(),
            id_abs: level62_transfer(&truth, geometry, TNOM_K, &vg, 1.0),
            v_ds: 1.0,
        };
        (device, output, second)
    })
}

#[test]
fn fitted_device_rejects_invalid_fit_and_input_without_mutating() {
    let vg: Vec<_> = (0..50).map(|i| i as f64 * 0.1).collect();
    assert!(matches!(
        FittedDevice::fit("flat".into(), vg.clone(), vec![0.0; vg.len()]),
        Err(FitDeviceError::NoExtractableAboveThreshold)
    ));

    let mut device = aostft_device();
    let original_geometry = device.geometry();
    let original_bias = device.bias();
    assert_eq!(
        device.set_geometry(GeometryParams {
            w_um: 0.0,
            l_um: 10.0,
        }),
        Err(InputError::InvalidGeometry)
    );
    assert_eq!(device.geometry(), original_geometry);
    assert_eq!(
        device.set_bias(0.0, original_bias.cox),
        Err(InputError::InvalidBias)
    );
    assert_eq!(device.bias(), original_bias);
    assert_eq!(device.set_cox(f64::NAN), Err(InputError::InvalidBias));
    assert_eq!(device.bias(), original_bias);
    assert_eq!(
        device.set_cox_from_accumulation(f64::NAN),
        Err(InputError::InvalidAccumulationCapacitance)
    );
}

#[test]
fn fitted_device_rejects_malformed_paired_transfer_samples() {
    let points = aostft_device().measured_points();
    let (vg, id): (Vec<_>, Vec<_>) = points.into_iter().map(|point| (point[0], point[1])).unzip();

    let mut short_id = id.clone();
    short_id.pop();
    assert!(matches!(
        FittedDevice::fit("mismatched".into(), vg.clone(), short_id),
        Err(FitDeviceError::InvalidTransferSamples)
    ));

    let mut non_finite_vg = vg.clone();
    non_finite_vg[10] = f64::NAN;
    assert!(matches!(
        FittedDevice::fit("non-finite-vg".into(), non_finite_vg, id.clone()),
        Err(FitDeviceError::InvalidTransferSamples)
    ));

    let mut non_finite_id = id;
    non_finite_id[10] = f64::INFINITY;
    assert!(matches!(
        FittedDevice::fit("non-finite-id".into(), vg, non_finite_id),
        Err(FitDeviceError::InvalidTransferSamples)
    ));
}

#[test]
fn cox_only_updates_preserve_extracted_dc_state() {
    let mut device = aostft_device();
    let aostft_before = *device.aostft_fit();
    let level62_before = device.level62().cloned();
    let subthreshold_before = device.subthreshold();
    let aostft_r2_before = device.model(FitModel::Aostft).r2();
    let level62_r2_before = device.model(FitModel::Level62).r2();

    device.set_cox(7.5e-4).expect("valid Cox commits");
    assert_eq!(device.bias().cox, 7.5e-4);
    assert_eq!(*device.aostft_fit(), aostft_before);
    assert_eq!(device.level62(), level62_before.as_ref());
    assert_eq!(device.subthreshold(), subthreshold_before);
    assert_eq!(device.model(FitModel::Aostft).r2(), aostft_r2_before);
    assert_eq!(device.model(FitModel::Level62).r2(), level62_r2_before);

    device
        .set_bias(device.bias().v_ds, 8.25e-4)
        .expect("same-VDS bias update takes the Cox-only path");
    assert_eq!(device.bias().cox, 8.25e-4);
    assert_eq!(*device.aostft_fit(), aostft_before);
    assert_eq!(device.level62(), level62_before.as_ref());
    assert_eq!(device.subthreshold(), subthreshold_before);
    assert_eq!(device.model(FitModel::Aostft).r2(), aostft_r2_before);
    assert_eq!(device.model(FitModel::Level62).r2(), level62_r2_before);

    let geometry = device.geometry();
    let area = geometry.w_um * 1.0e-6 * geometry.l_um * 1.0e-6;
    let derived = device
        .set_cox_from_accumulation(9.0e-4 * area)
        .expect("valid accumulation capacitance commits");
    assert_close(derived, 9.0e-4, 1.0e-14);
    assert_eq!(*device.aostft_fit(), aostft_before);
    assert_eq!(device.level62(), level62_before.as_ref());
    assert_eq!(device.subthreshold(), subthreshold_before);
    assert_eq!(device.model(FitModel::Aostft).r2(), aostft_r2_before);
    assert_eq!(device.model(FitModel::Level62).r2(), level62_r2_before);
}

#[test]
fn geometry_updates_preserve_the_geometry_independent_aostft_fit() {
    let mut device = aostft_device();
    assert_eq!(
        device
            .replace_output(output_family(device.aostft_fit().vt))
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let fit_before = *device.aostft_fit();
    let output_before = device.output();
    let subthreshold_before = device.subthreshold();
    let r2_before = device.model(FitModel::Aostft).r2();

    device
        .set_geometry(GeometryParams {
            w_um: 80.0,
            l_um: 12.0,
        })
        .expect("valid geometry commits");

    assert_eq!(*device.aostft_fit(), fit_before);
    assert_eq!(device.output(), output_before);
    assert_eq!(device.subthreshold(), subthreshold_before);
    assert_eq!(device.model(FitModel::Aostft).r2(), r2_before);
}

#[test]
fn manual_aostft_domains_are_validated_atomically() {
    let mut device = aostft_device();
    let original_fit = *device.aostft_fit();
    let original_output = device.output();

    for params in [
        ModelParams {
            vt: original_fit.vt,
            gamma: -1.0,
            k: original_fit.k,
        },
        ModelParams {
            vt: original_fit.vt,
            gamma: original_fit.gamma,
            k: 0.0,
        },
    ] {
        assert_eq!(
            device.set_aostft_fit(params),
            Err(EditError::InvalidAostftFit)
        );
        assert_eq!(*device.aostft_fit(), original_fit);
        assert!(!device.model(FitModel::Aostft).is_manual());
    }

    let defaults = OutputParams::card_defaults();
    for params in [
        OutputParams {
            alpha_sat: 0.0,
            ..defaults
        },
        OutputParams {
            lambda: -1.0,
            ..defaults
        },
        OutputParams { m: 0.0, ..defaults },
        OutputParams {
            alpha_sat: f64::INFINITY,
            ..defaults
        },
    ] {
        assert_eq!(
            device.set_aostft_output(params),
            Err(EditError::InvalidOutput)
        );
        assert_eq!(*device.aostft_fit(), original_fit);
        assert_eq!(device.output(), original_output);
        assert!(!device.model(FitModel::Aostft).is_manual());
    }
}

#[test]
fn mapped_aostft_edits_must_remain_reversible() {
    let mut device = aostft_device();
    assert_eq!(
        device
            .replace_output(output_family(device.aostft_fit().vt))
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    device
        .set_aostft_output(OutputParams {
            alpha_sat: 100.0,
            lambda: 0.0,
            m: 2.5,
        })
        .expect("representable mapped output");
    let before = *device.aostft_fit();

    assert_eq!(
        device.set_aostft_fit(ModelParams {
            vt: before.vt,
            gamma: before.gamma,
            k: f64::MAX,
        }),
        Err(EditError::InvalidAostftCardMapping)
    );
    assert_eq!(*device.aostft_fit(), before);
    assert!(
        device.detach_output().is_ok(),
        "a rejected edit must leave the accepted mapping reversible"
    );
}

#[test]
fn manual_level62_domains_are_validated_atomically() {
    let mut device = aostft_device();
    let original = device.level62().expect("Level 62 fit").params;
    let l_m = device.geometry().l_um * 1.0e-6;
    let temperature_mobility_coefficient = Level62Params {
        tnom_k: TNOM_K + 2.0,
        dmu1: original.mu1,
        ..original
    };
    let derived_asat_failure = Level62Params {
        lasat: 2.0 * original.asat * l_m,
        ..original
    };
    let temperature_threshold_coefficient = Level62Params {
        tnom_k: 1.0,
        dvto: f64::MAX,
        ..original
    };
    let dielectric_ratio_failure = Level62Params {
        epsi: f64::MAX,
        tox: f64::MIN_POSITIVE,
        ..original
    };
    let invalid = [
        Level62Params {
            vto: f64::NAN,
            ..original
        },
        Level62Params {
            mu0: 0.0,
            ..original
        },
        Level62Params {
            mu1: 0.0,
            ..original
        },
        Level62Params {
            mmu: 0.0,
            ..original
        },
        Level62Params {
            mus: 0.0,
            ..original
        },
        Level62Params {
            asat: 0.0,
            ..original
        },
        Level62Params {
            lambda: -1.0,
            ..original
        },
        Level62Params {
            delta: 0.0,
            ..original
        },
        Level62Params {
            eta: 0.0,
            ..original
        },
        Level62Params {
            vkink: 0.0,
            ..original
        },
        Level62Params {
            lkink: -1.0,
            ..original
        },
        Level62Params {
            mk: 0.0,
            ..original
        },
        Level62Params {
            i00: -1.0,
            ..original
        },
        Level62Params {
            eb: -1.0,
            ..original
        },
        Level62Params {
            eps: 0.0,
            ..original
        },
        Level62Params {
            epsi: 0.0,
            ..original
        },
        Level62Params {
            tox: 0.0,
            ..original
        },
        Level62Params {
            rs: -1.0,
            ..original
        },
        Level62Params {
            rd: -1.0,
            ..original
        },
        Level62Params {
            at: -1.0,
            ..original
        },
        Level62Params {
            bt: -1.0,
            ..original
        },
        Level62Params {
            vsi: 0.0,
            ..original
        },
        Level62Params {
            lasat: -1.0,
            ..original
        },
        Level62Params {
            tnom_k: 0.0,
            ..original
        },
        derived_asat_failure,
        dielectric_ratio_failure,
    ];

    for params in invalid {
        assert_eq!(
            device.set_level62_params(params),
            Err(EditError::InvalidLevel62Params)
        );
        assert_eq!(device.level62().expect("fit remains").params, original);
        assert!(!device.model(FitModel::Level62).is_manual());
    }

    for params in [
        temperature_mobility_coefficient,
        temperature_threshold_coefficient,
    ] {
        device
            .set_level62_params(params)
            .expect("finite temperature coefficients are nominal at their own TNOM");
        assert_eq!(device.level62().expect("fit remains").params, params);
        assert!(device.model(FitModel::Level62).is_manual());
    }
}

#[test]
fn geometry_rejects_a_manual_level62_combination_outside_its_domain() {
    let mut device = aostft_device();
    let geometry = device.geometry();
    let mut params = device.level62().expect("Level 62 fit").params;
    params.lasat = params.asat * geometry.l_um * 1.0e-6 * 0.5;
    device
        .set_level62_params(params)
        .expect("parameters are valid at the current length");
    let before = device.level62().expect("manual fit").clone();

    assert_eq!(
        device.set_geometry(GeometryParams {
            l_um: geometry.l_um * 0.25,
            ..geometry
        }),
        Err(InputError::InvalidGeometry)
    );
    assert_eq!(device.geometry(), geometry);
    assert_eq!(device.level62(), Some(&before));
}

#[test]
fn manual_level62_r2_tracks_geometry_and_drain_bias_changes() {
    let mut device = aostft_device();
    let params = device.level62().expect("Level 62 fit").params;
    device
        .set_level62_params(params)
        .expect("manual parameters commit");

    let mut geometry = device.geometry();
    geometry.w_um *= 1.7;
    device.set_geometry(geometry).expect("valid geometry");
    let geometry_r2 = device.model(FitModel::Level62).r2();
    device
        .set_level62_params(params)
        .expect("same parameters recompute the reference score");
    assert_eq!(device.model(FitModel::Level62).r2(), geometry_r2);

    let bias = device.bias();
    device
        .set_bias(bias.v_ds * 1.3, bias.cox)
        .expect("valid drain bias");
    let bias_r2 = device.model(FitModel::Level62).r2();
    device
        .set_level62_params(params)
        .expect("same parameters recompute the reference score");
    assert_eq!(device.model(FitModel::Level62).r2(), bias_r2);
}

#[test]
fn output_attach_detach_restores_the_transfer_fit_and_keeps_replacements_recoverable() {
    let mut device = aostft_device();
    let before = *device.aostft_fit();
    // The lifecycle must preserve manual models while an output is attached; this
    // also isolates the H/card mapping from the optimizers covered elsewhere.
    device
        .set_aostft_fit(ModelParams {
            vt: before.vt,
            gamma: before.gamma,
            k: before.k,
        })
        .expect("manual AOSTFT fit");
    let level62 = device.level62().expect("Level 62 fit").params;
    device
        .set_level62_params(level62)
        .expect("manual Level 62 fit");
    let first = output_family(2.0);
    assert_eq!(
        device
            .replace_output(first.clone())
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    assert!(device.has_output());
    assert!(device.has_output_curves());

    let replacement = output_family(2.0);
    assert_eq!(
        device
            .replace_output(replacement.clone())
            .expect("output replacement preserves DIBL")
            .displaced,
        first
    );
    assert_eq!(
        device.detach_output().expect("attached output"),
        replacement
    );
    assert!(!device.has_output());
    assert!(!device.has_output_curves());
    let after = device.aostft_fit();
    assert!((after.vt - before.vt).abs() < 1.0e-10);
    assert!((after.gamma - before.gamma).abs() < 1.0e-10);
    assert!((after.k - before.k).abs() < 1.0e-16);
}

#[test]
fn manual_output_replacement_remaps_from_the_same_h_fit_and_round_trips() {
    let mut device = aostft_device();
    let h_fit = *device.aostft_fit();
    device
        .set_aostft_fit(ModelParams {
            vt: h_fit.vt,
            gamma: h_fit.gamma,
            k: h_fit.k,
        })
        .expect("manual H fit");
    let transfer_before = device.model(FitModel::Aostft).transfer_overlay();
    let first = OutputParams {
        alpha_sat: 0.55,
        lambda: 0.005,
        m: 2.0,
    };
    assert_eq!(
        device
            .replace_output(output_family_with_params(h_fit.vt, first))
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let first_extracted = device.output().expect("first output extracts");
    let first_strict = *device.aostft_fit();
    let (first_gamma, first_k) = recover_h_fit(first_strict, first_extracted, device.bias().v_ds);
    assert_close(first_gamma, h_fit.gamma, 1.0e-14);
    assert_close(first_k, h_fit.k, 1.0e-14);

    let second = OutputParams {
        alpha_sat: 0.95,
        lambda: 0.035,
        m: 4.0,
    };
    assert_eq!(
        device
            .replace_output(output_family_with_params(h_fit.vt, second))
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let second_extracted = device.output().expect("replacement output extracts");
    assert_ne!(second_extracted, first_extracted);
    let second_strict = *device.aostft_fit();
    let (second_gamma, second_k) =
        recover_h_fit(second_strict, second_extracted, device.bias().v_ds);
    assert_close(second_gamma, h_fit.gamma, 1.0e-14);
    assert_close(second_k, h_fit.k, 1.0e-14);
    assert_ne!(
        second_strict.k, first_strict.k,
        "a different output mapping must rescale the stored strict-card gain"
    );

    device
        .detach_output()
        .expect("replacement remains detachable");
    let restored = *device.aostft_fit();
    assert_close(restored.gamma, h_fit.gamma, 1.0e-14);
    assert_close(restored.k, h_fit.k, 1.0e-14);
    for (actual, expected) in device
        .model(FitModel::Aostft)
        .transfer_overlay()
        .iter()
        .zip(&transfer_before)
    {
        assert_eq!(actual[0], expected[0]);
        assert_close(actual[1], expected[1], 1.0e-12);
    }
}

#[test]
fn manual_mapped_bias_changes_remap_through_h_before_commit() {
    let mut device = aostft_device();
    let h_fit = *device.aostft_fit();
    let output = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.04,
        m: 2.5,
    };
    device
        .set_aostft_output(output)
        .expect("manual output maps to a strict card");
    let old_card = *device.aostft_fit();
    let old_bias = device.bias();
    let new_v_ds = old_bias.v_ds * 1.75;
    device
        .set_bias(new_v_ds, old_bias.cox)
        .expect("candidate bias is representable");

    let strict = *device.aostft_fit();
    assert_ne!(
        strict.k, old_card.k,
        "the cached card projection must be rebuilt for the new drain bias"
    );
    let (recovered_gamma, recovered_k) = recover_h_fit(strict, output, new_v_ds);
    assert_close(recovered_gamma, h_fit.gamma, 1.0e-14);
    assert_close(recovered_k, h_fit.k, 1.0e-14);
    let expected_k = h_fit.k * (new_v_ds / (output.alpha_sat * (1.0 + output.lambda * new_v_ds)));
    assert_close(strict.k, expected_k, 1.0e-14);
}

#[test]
fn aostft_attach_detach_bias_manual_reset_sequence_preserves_public_projection() {
    let mut sequenced = aostft_device();
    let initial_h = *sequenced.aostft_fit();
    let curves = output_family(initial_h.vt);

    assert_eq!(
        sequenced
            .replace_output(curves.clone())
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let attached_output = sequenced.output().expect("output fit");
    let attached_card = *sequenced.aostft_fit();
    let (recovered_gamma, recovered_k) =
        recover_h_fit(attached_card, attached_output, sequenced.bias().v_ds);
    assert_close(recovered_gamma, initial_h.gamma, 1.0e-14);
    assert_close(recovered_k, initial_h.k, 1.0e-14);

    assert_eq!(
        sequenced.detach_output().expect("attached output detaches"),
        curves
    );
    assert_close(sequenced.aostft_fit().gamma, initial_h.gamma, 1.0e-14);
    assert_close(sequenced.aostft_fit().k, initial_h.k, 1.0e-14);

    let next_v_ds = sequenced.bias().v_ds * 1.6;
    sequenced
        .set_bias(next_v_ds, sequenced.bias().cox)
        .expect("bias commits after detach");
    let mut direct = aostft_device();
    direct
        .set_bias(next_v_ds, direct.bias().cox)
        .expect("history-free control bias commits");

    let manual_output = OutputParams {
        alpha_sat: 0.8,
        lambda: 0.02,
        m: 3.0,
    };
    let manual_card = ModelParams {
        vt: 1.25,
        gamma: -0.25,
        k: 2.5e-6,
    };
    for device in [&mut sequenced, &mut direct] {
        device
            .set_aostft_output(manual_output)
            .expect("manual output commits");
        device
            .set_aostft_fit(manual_card.clone())
            .expect("manual strict-card edit commits");

        assert_close(device.aostft_fit().vt, manual_card.vt, 1.0e-14);
        assert_close(device.aostft_fit().gamma, manual_card.gamma, 1.0e-14);
        assert_close(device.aostft_fit().k, manual_card.k, 1.0e-14);
        assert!(device.model(FitModel::Aostft).is_manual());

        let card = device
            .model(FitModel::Aostft)
            .export_artifact()
            .expect("manual AOSTFT exports")
            .text;
        for golden_line in [
            "parameter real VTO = 1.2500e0;",
            "parameter real GAMMA = -2.5000e-1;",
            "parameter real ALPHASAT = 8.0000e-1;",
            "parameter real LAMBDA = 2.0000e-2;",
            "parameter real MSAT = 3.0000e0;",
        ] {
            assert!(
                card.contains(golden_line),
                "manual card must retain `{golden_line}`"
            );
        }
        let geometry = device.geometry();
        let expected_kp = manual_card.k / next_v_ds * geometry.l_um / geometry.w_um;
        let golden_kp = format!("parameter real KP = {expected_kp:.4e};");
        assert!(
            card.contains(&golden_kp),
            "manual card must retain `{golden_kp}`"
        );
    }

    assert_eq!(
        sequenced
            .model(FitModel::Aostft)
            .export_artifact()
            .expect("sequenced card"),
        direct
            .model(FitModel::Aostft)
            .export_artifact()
            .expect("history-free card"),
        "card output must not depend on prior attach/detach history"
    );
    assert_eq!(
        sequenced.model(FitModel::Aostft).transfer_overlay(),
        direct.model(FitModel::Aostft).transfer_overlay(),
        "forward output must use the same strict-card projection as export"
    );

    for device in [&mut sequenced, &mut direct] {
        device
            .reset_autofit(FitModel::Aostft)
            .expect("automatic H fit restores");
        assert!(!device.model(FitModel::Aostft).is_manual());
        assert_eq!(device.output(), None);
    }
    assert_eq!(sequenced.aostft_fit(), direct.aostft_fit());
    assert_eq!(
        sequenced.model(FitModel::Aostft).transfer_overlay(),
        direct.model(FitModel::Aostft).transfer_overlay()
    );
    assert_eq!(
        sequenced
            .model(FitModel::Aostft)
            .export_artifact()
            .expect("sequenced reset card"),
        direct
            .model(FitModel::Aostft)
            .export_artifact()
            .expect("history-free reset card")
    );
}

#[test]
fn output_then_dibl_refinement_is_reapplied_after_geometry_rebuild() {
    let (mut device, output, second) = level62_measurements();
    let geometry = device.geometry();
    assert_eq!(
        device
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let replacement = device
        .replace_second_transfer(second)
        .expect("DIBL pair refines");
    assert!(replacement.at > 0.0);
    assert!(replacement.displaced.is_none());

    device
        .set_geometry(GeometryParams {
            l_um: 10.5,
            ..geometry
        })
        .expect("geometry rebuilds");
    let refined = device.level62().expect("Level 62 stays available").params;
    assert!(
        refined.at > 0.0,
        "DIBL must be reapplied after output refinement"
    );
    assert!(
        refined.lambda > 0.0,
        "output refinement must survive rebuild"
    );
}

#[test]
fn successful_dibl_replacement_returns_the_exact_previous_measurement() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());

    let first = device
        .replace_second_transfer(second.clone())
        .expect("first DIBL measurement fits");
    assert!(first.at > 0.0);
    assert!(first.displaced.is_none());

    let replacement = device
        .replace_second_transfer(second.clone())
        .expect("replacement DIBL measurement fits");
    assert!(replacement.at > 0.0);
    assert_eq!(replacement.displaced, Some(second.clone()));
    assert_eq!(
        device
            .detach_second_transfer()
            .expect("replacement remains attached"),
        second
    );
}

#[test]
fn level62_rebuild_is_repeatable_and_independent_of_attachment_order() {
    let (mut output_first, output, second) = level62_measurements();
    assert_eq!(
        output_first
            .replace_output(output.clone())
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    assert!(output_first
        .replace_second_transfer(second.clone())
        .expect("DIBL fits after output")
        .displaced
        .is_none());
    let expected = output_first.level62().expect("combined fit").params;

    assert_eq!(
        output_first
            .replace_second_transfer(second.clone())
            .expect("reloading the same DIBL pair is idempotent")
            .displaced,
        Some(second.clone())
    );
    assert_eq!(
        output_first.level62().expect("repeated DIBL fit").params,
        expected
    );
    assert_eq!(
        output_first
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    assert_eq!(
        output_first.level62().expect("repeated output fit").params,
        expected
    );

    let (mut dibl_first, output, second) = level62_measurements();
    assert!(dibl_first
        .replace_second_transfer(second)
        .expect("DIBL fits before output")
        .displaced
        .is_none());
    assert_eq!(
        dibl_first
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    assert_eq!(
        dibl_first.level62().expect("reordered combined fit").params,
        expected
    );
}

#[test]
fn failed_dibl_replacement_preserves_the_previous_attachment_and_fit() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second.clone())
        .expect("initial DIBL pair fits")
        .displaced
        .is_none());
    let rejected = SecondTransfer {
        vg: second.vg,
        id_abs: second.id_abs.into_iter().rev().collect(),
        v_ds: 1.0,
    };
    let before = device.clone();

    assert_eq!(
        device.replace_second_transfer(rejected.clone()),
        Err(DiblReplacementError {
            reason: DiblError::NoImprovement,
            rejected,
        })
    );
    assert_eq!(device, before);

    let mut geometry = device.geometry();
    geometry.l_um += 0.5;
    device.set_geometry(geometry).expect("geometry rebuilds");
    assert!(
        device.level62().expect("fit remains available").params.at > 0.0,
        "the previously accepted DIBL measurement remains attached"
    );
}

#[test]
fn malformed_second_transfer_is_rejected_without_mutating_the_device() {
    let (device, _, valid) = level62_measurements();
    let mut mismatched = valid.clone();
    mismatched.id_abs.pop();
    let too_short = SecondTransfer {
        vg: valid.vg[..9].to_vec(),
        id_abs: valid.id_abs[..9].to_vec(),
        v_ds: valid.v_ds,
    };
    let mut non_finite_vg = valid.clone();
    non_finite_vg.vg[0] = f64::NAN;
    let mut non_finite_id = valid.clone();
    non_finite_id.id_abs[0] = f64::INFINITY;
    let mut zero_bias = valid.clone();
    zero_bias.v_ds = 0.0;
    let mut non_finite_bias = valid;
    non_finite_bias.v_ds = f64::NAN;

    for malformed in [
        mismatched,
        too_short,
        non_finite_vg,
        non_finite_id,
        zero_bias,
        non_finite_bias,
    ] {
        let mut candidate = device.clone();
        let before = candidate.clone();
        let rejected = malformed.clone();
        let error = candidate
            .replace_second_transfer(malformed)
            .expect_err("malformed transfer must be rejected");
        assert_eq!(error.reason, DiblError::InvalidSecondTransfer);
        assert!(
            error.rejected.vg.len() == rejected.vg.len()
                && error.rejected.id_abs.len() == rejected.id_abs.len()
                && error
                    .rejected
                    .vg
                    .iter()
                    .zip(&rejected.vg)
                    .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
                && error
                    .rejected
                    .id_abs
                    .iter()
                    .zip(&rejected.id_abs)
                    .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
                && error.rejected.v_ds.to_bits() == rejected.v_ds.to_bits(),
            "rejection must return the exact proposed float payload"
        );
        assert_eq!(candidate, before);
    }
}

#[test]
fn detaching_dibl_restores_the_output_only_level62_fit() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());
    let output_only = device.level62().expect("output-refined fit").params;
    assert!(!device.has_second_transfer());

    assert!(device
        .replace_second_transfer(second.clone())
        .expect("DIBL pair fits")
        .displaced
        .is_none());
    assert!(device.has_second_transfer());
    assert_ne!(
        device.level62().expect("DIBL-refined fit").params,
        output_only
    );

    assert_eq!(
        device
            .detach_second_transfer()
            .expect("attached DIBL measurement detaches"),
        second
    );
    assert!(!device.has_second_transfer());
    assert_eq!(
        device.level62().expect("output-only fit restored").params,
        output_only
    );
    assert_eq!(
        device.detach_second_transfer(),
        Err(DetachDiblError::NoSecondTransfer)
    );
}

#[test]
fn manual_level62_detach_keeps_the_edit_but_reset_no_longer_reapplies_dibl() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output.clone())
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second)
        .expect("DIBL pair fits")
        .displaced
        .is_none());
    let mut manual = device.level62().expect("DIBL-refined fit").params;
    manual.vto += 0.125;
    device
        .set_level62_params(manual)
        .expect("manual Level 62 edit");

    device
        .detach_second_transfer()
        .expect("manual model may release its measurement");
    assert!(!device.has_second_transfer());
    assert_eq!(device.level62().expect("manual fit remains").params, manual);

    device
        .reset_autofit(FitModel::Level62)
        .expect("output-only auto-fit restores");
    let (mut output_only, _, _) = level62_measurements();
    assert!(output_only
        .replace_output(output)
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());
    assert_eq!(
        device.level62().expect("reset fit").params,
        output_only.level62().expect("output-only control").params
    );
}

#[test]
fn lazy_analog_quality_does_not_change_fitted_device_equality() {
    let device = aostft_device();
    let untouched = device.clone();

    let _ = device.model(FitModel::Aostft).analog_fit_quality();

    assert_eq!(
        device, untouched,
        "a read-through cache is not part of scientific identity"
    );
}

#[test]
fn replacing_output_with_an_unfittable_family_clears_stale_level62_output_terms() {
    let (mut device, output, _) = level62_measurements();
    let (base, _, _) = level62_measurements();
    let base_params = base.level62().expect("base fit").params;
    assert_eq!(
        device
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    assert_ne!(
        device.level62().expect("output-refined fit").params,
        base_params
    );

    assert_eq!(
        device
            .replace_output(Vec::new())
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::NoFit
    );
    assert_eq!(
        device.level62().expect("base fit restored").params,
        base_params
    );
}

#[test]
fn manual_aostft_parameters_survive_rebuild_then_reset_to_autofit() {
    let mut device = aostft_device();
    let baseline = *device.aostft_fit();
    let edited = ModelParams {
        vt: baseline.vt + 1.0,
        gamma: baseline.gamma,
        k: baseline.k,
    };
    device.set_aostft_fit(edited.clone()).expect("manual edit");
    assert!(device.model(FitModel::Aostft).is_manual());
    device
        .set_geometry(GeometryParams {
            w_um: 33.0,
            l_um: 7.0,
        })
        .expect("geometry rebuild");
    assert_eq!(device.aostft_fit().vt, edited.vt);

    device
        .reset_autofit(FitModel::Aostft)
        .expect("auto-fit restores");
    assert!(!device.model(FitModel::Aostft).is_manual());
    assert!((device.aostft_fit().vt - baseline.vt).abs() < 1.0e-10);
    assert_eq!(
        device.set_aostft_fit(ModelParams {
            vt: f64::NAN,
            ..edited
        }),
        Err(EditError::NonFiniteAostftFit)
    );
}

#[test]
fn manual_level62_parameters_survive_rebuild_then_reset_to_autofit() {
    let mut device = aostft_device();
    let mut edited = device.level62().expect("Level 62 fit").params;
    edited.vto += 1.234;
    edited.tnom_k = 310.0;
    device
        .set_level62_params(edited)
        .expect("manual Level 62 edit");
    let card = device
        .model(FitModel::Level62)
        .export_artifact()
        .expect("manual fit exports")
        .text;
    assert!(
        card.contains("parameter real TNOM_K = 3.1000e2"),
        "export must use the edited Level 62 measurement temperature"
    );
    let expected = {
        let vg = device
            .measured_points()
            .into_iter()
            .map(|point| device.polarity().map_vg(point[0]))
            .collect::<Vec<_>>();
        level62_transfer(
            &edited,
            device.geometry(),
            edited.tnom_k,
            &vg,
            device.bias().v_ds,
        )
    };
    assert_eq!(
        device
            .model(FitModel::Level62)
            .transfer_overlay()
            .into_iter()
            .map(|point| point[1])
            .collect::<Vec<_>>(),
        expected,
        "in-app Level 62 evaluates at its edited TNOM, matching the card's nominal state"
    );
    device
        .set_geometry(GeometryParams {
            w_um: 33.0,
            l_um: 7.0,
        })
        .expect("geometry rebuild");
    assert_eq!(
        device.level62().expect("manual fit remains").params.vto,
        edited.vto
    );
    assert!(device.model(FitModel::Level62).is_manual());

    device
        .reset_autofit(FitModel::Level62)
        .expect("auto-fit restores");
    assert!(!device.model(FitModel::Level62).is_manual());
    assert_ne!(device.level62().expect("auto fit").params.vto, edited.vto);
}

#[test]
fn dibl_rejects_a_second_transfer_without_a_bias_lever() {
    let mut device = aostft_device();
    let points = device.measured_points();
    let (vg, id): (Vec<_>, Vec<_>) = points.into_iter().map(|p| (p[0], p[1])).unzip();
    let rejected = SecondTransfer {
        vg,
        id_abs: id,
        v_ds: device.bias().v_ds,
    };
    let before = device.clone();
    assert_eq!(
        device.replace_second_transfer(rejected.clone()),
        Err(DiblReplacementError {
            reason: DiblError::BiasTooClose,
            rejected,
        })
    );
    assert_eq!(device, before);
}

#[test]
fn manual_or_unavailable_level62_rejection_returns_the_exact_second_transfer() {
    let (_, _, second) = level62_measurements();

    let mut manual = aostft_device();
    manual
        .set_level62_params(manual.level62().expect("Level 62 fit").params)
        .expect("manual edit");
    let before = manual.clone();
    assert_eq!(
        manual.replace_second_transfer(second.clone()),
        Err(DiblReplacementError {
            reason: DiblError::Level62Manual,
            rejected: second.clone(),
        })
    );
    assert_eq!(manual, before);

    let params = ModelParams {
        vt: 2.0,
        gamma: 0.5,
        k: 1.0e-6,
    };
    let vg: Vec<_> = (0..8).map(|i| i as f64).collect();
    let mut unavailable =
        FittedDevice::fit("short".into(), vg.clone(), transfer_curve(&params, &vg))
            .expect("short transfer still fits AOSTFT");
    assert!(unavailable.level62().is_none());
    let before = unavailable.clone();
    assert_eq!(
        unavailable.replace_second_transfer(second.clone()),
        Err(DiblReplacementError {
            reason: DiblError::Level62Unavailable,
            rejected: second,
        })
    );
    assert_eq!(unavailable, before);
}

#[test]
fn retained_dibl_rejects_an_equal_primary_bias_without_mutation() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output fits before DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second.clone())
        .expect("DIBL attaches")
        .displaced
        .is_none());
    let before = device.clone();

    assert_eq!(
        device.set_bias(second.v_ds.abs(), device.bias().cox),
        Err(InputError::RetainedDiblNotApplied)
    );
    assert_eq!(device, before);
    assert!(device.is_second_transfer_applied());
}

#[test]
fn retained_dibl_rejects_an_incompatible_geometry_without_mutation() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output fits before DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second)
        .expect("DIBL attaches")
        .displaced
        .is_none());
    let before = device.clone();

    assert_eq!(
        device.set_geometry(GeometryParams {
            l_um: 1.0e-3,
            ..device.geometry()
        }),
        Err(InputError::RetainedDiblNotApplied)
    );
    assert_eq!(device, before);
    assert!(device.is_second_transfer_applied());
}

#[test]
fn conflicting_output_replacement_returns_exact_curves_without_mutation() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output.clone())
        .expect("output fits before DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second)
        .expect("DIBL attaches")
        .displaced
        .is_none());
    let mut proposed = output;
    for curve in &mut proposed {
        for id in &mut curve.id {
            *id *= 1.0e-6;
        }
    }
    let before = device.clone();

    assert_eq!(
        device.replace_output(proposed.clone()),
        Err(OutputReplacementError::RetainedDiblNotApplied { rejected: proposed })
    );
    assert_eq!(device, before);
    assert!(device.is_second_transfer_applied());
}

#[test]
fn manual_level62_retains_but_suspends_dibl_until_a_successful_reset() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output fits before DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second)
        .expect("DIBL attaches")
        .displaced
        .is_none());
    assert!(device.is_second_transfer_applied());

    let params = device.level62().expect("DIBL-refined fit").params;
    device
        .set_level62_params(params)
        .expect("manual Level 62 parameters commit");
    assert!(device.has_second_transfer());
    assert!(!device.is_second_transfer_applied());

    device
        .reset_autofit(FitModel::Level62)
        .expect("unchanged measurements reapply DIBL");
    assert!(device.is_second_transfer_applied());
}

#[test]
fn failed_manual_reset_keeps_the_retained_dibl_and_manual_science() {
    let (mut device, output, second) = level62_measurements();
    assert!(device
        .replace_output(output)
        .expect("output fits before DIBL")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second.clone())
        .expect("DIBL attaches")
        .displaced
        .is_none());
    let params = device.level62().expect("DIBL-refined fit").params;
    device
        .set_level62_params(params)
        .expect("manual Level 62 parameters commit");
    device
        .set_bias(second.v_ds.abs(), device.bias().cox)
        .expect("manual mode may retain an inactive DIBL measurement");
    let before = device.clone();

    assert_eq!(
        device.reset_autofit(FitModel::Level62),
        Err(RefitError::RetainedDiblNotApplied)
    );
    assert_eq!(device, before);
    assert!(device.has_second_transfer());
    assert!(!device.is_second_transfer_applied());
}

#[test]
fn conflicting_output_detach_preserves_both_attachments() {
    let (mut device, mut output, mut second) = level62_measurements();
    for curve in &mut output {
        for id in &mut curve.id {
            *id *= 1.0e3;
        }
    }
    for id in &mut second.id_abs {
        *id *= 10.0;
    }
    assert!(device
        .replace_output(output)
        .expect("output family fits")
        .displaced
        .is_empty());
    assert!(device
        .replace_second_transfer(second)
        .expect("this DIBL measurement needs the output-refined candidate")
        .displaced
        .is_none());
    let before = device.clone();

    assert_eq!(
        device.detach_output(),
        Err(DetachOutputError::RetainedDiblNotApplied)
    );
    assert_eq!(device, before);
    assert!(device.has_output_curves());
    assert!(device.is_second_transfer_applied());
}
