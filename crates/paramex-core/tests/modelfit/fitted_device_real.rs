//! Real-fixture regressions at the core fitted-device interface.

use std::path::PathBuf;

use super::{clone_fixture, FixtureCache};
use paramex_core::modelfit::{
    parse_output_file, FitModel, FittedDevice, InputError, ModelParams, OutputAttachOutcome,
    OutputParams, OutputSeries, Polarity,
};
use paramex_core::transfer::parse_transfer_file;

fn real_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/modelfit")
        .join(name)
}

fn real_pair() -> FittedDevice {
    static FIXTURE: FixtureCache<FittedDevice> = FixtureCache::new();

    clone_fixture(&FIXTURE, || {
        let output = parse_output_file(&real_fixture("2-6o.xlsx")).expect("real output fixture");
        let mut device = real_transfer_only();
        assert_eq!(
            device
                .replace_output(output)
                .expect("output replacement preserves DIBL")
                .outcome,
            OutputAttachOutcome::Fitted
        );
        device
    })
}

fn real_transfer_only() -> FittedDevice {
    folder_transfer("2-6.xlsx")
}

fn folder_transfer(name: &'static str) -> FittedDevice {
    static FILE_1_1: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_1227_3_3: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_1227_3_5: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_2_6: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_3_52: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_4_1: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_7_2: FixtureCache<FittedDevice> = FixtureCache::new();
    static FILE_7_3: FixtureCache<FittedDevice> = FixtureCache::new();

    let cache = match name {
        "1-1.xlsx" => &FILE_1_1,
        "1227-3-3.xlsx" => &FILE_1227_3_3,
        "1227-3-5.xlsx" => &FILE_1227_3_5,
        "2-6.xlsx" => &FILE_2_6,
        "3-52.xlsx" => &FILE_3_52,
        "4-1.xlsx" => &FILE_4_1,
        "7-2.xlsx" => &FILE_7_2,
        "7-3.xlsx" => &FILE_7_3,
        _ => panic!("no fitted-device cache declared for {name}"),
    };

    clone_fixture(cache, || {
        let transfer = parse_transfer_file(&real_fixture(name)).expect("folder transfer fixture");
        FittedDevice::fit(transfer.name, transfer.vg, transfer.id_abs).unwrap_or_else(|_| {
            panic!("{name} must remain visible even when its automatic fit is poor")
        })
    })
}

fn worst_gm_drop(mut gm: Vec<[f64; 2]>, polarity: Polarity) -> f64 {
    gm.sort_by(|a, b| polarity.map_vg(a[0]).total_cmp(&polarity.map_vg(b[0])));
    let peak = gm.iter().map(|point| point[1]).fold(0.0_f64, f64::max);
    let mut running = 0.0_f64;
    let mut worst = 0.0_f64;
    for point in gm.into_iter().filter(|point| point[1] >= 0.05 * peak) {
        running = running.max(point[1]);
        worst = worst.max((running - point[1]) / running);
    }
    worst
}

fn nearest(points: &[[f64; 2]], x: f64) -> [f64; 2] {
    *points
        .iter()
        .min_by(|a, b| (a[0] - x).abs().total_cmp(&(b[0] - x).abs()))
        .expect("non-empty points")
}

fn overlay_log_r2(modelled: &[[f64; 2]], measured: &[[f64; 2]]) -> f64 {
    let pairs: Vec<_> = modelled
        .iter()
        .zip(measured)
        .filter_map(|(p, m)| (p[1] > 0.0 && m[1] > 0.0).then_some((p[1].log10(), m[1].log10())))
        .collect();
    let mean = pairs.iter().map(|(_, m)| m).sum::<f64>() / pairs.len() as f64;
    let total = pairs.iter().map(|(_, m)| (m - mean).powi(2)).sum::<f64>();
    let residual = pairs.iter().map(|(p, m)| (m - p).powi(2)).sum::<f64>();
    1.0 - residual / total
}

#[test]
fn aostft_uses_one_forward_at_the_transfer_bias() {
    let device = real_pair();
    let view = device.model(FitModel::Aostft);
    let transfer = view.transfer_overlay();
    let output = view.output_family();

    for family in output.iter().filter(|family| family.vg <= -1.0) {
        let transfer_at_gate = nearest(&transfer, family.vg)[1];
        let output_at_bias = nearest(&family.modelled, 5.0)[1];
        let relative = (transfer_at_gate - output_at_bias).abs() / transfer_at_gate;
        assert!(
            relative < 1.0e-10,
            "Vg={} transfer={transfer_at_gate:e}, output={output_at_bias:e}",
            family.vg
        );
    }

    let peak = output
        .iter()
        .flat_map(|family| family.measured.iter().map(|point| point[1]))
        .fold(0.0_f64, f64::max);
    let errors: Vec<_> = output
        .iter()
        .flat_map(|family| family.measured.iter().zip(&family.modelled))
        .filter(|(measured, _)| measured[0] > 0.2)
        .map(|(measured, modelled)| (measured[1] - modelled[1]).powi(2))
        .collect();
    let nrmse = (errors.iter().sum::<f64>() / errors.len() as f64).sqrt() / peak;
    assert!(nrmse < 0.05, "AOSTFT output NRMSE={nrmse:.3}");
}

#[test]
fn aostft_reports_the_overlay_r2_and_has_no_crossover_gm_notch() {
    let device = real_pair();
    let view = device.model(FitModel::Aostft);
    let expected_r2 = overlay_log_r2(&view.transfer_overlay(), &device.measured_points());
    assert!(
        (device.aostft_fit().r2 - expected_r2).abs() < 1.0e-12,
        "displayed R2={} overlay R2={expected_r2}",
        device.aostft_fit().r2
    );

    let mut gm = view.gm_series();
    gm.sort_by(|a, b| {
        device
            .polarity()
            .map_vg(a[0])
            .total_cmp(&device.polarity().map_vg(b[0]))
    });
    let maximum = gm.iter().map(|point| point[1]).fold(0.0_f64, f64::max);
    let mut running = 0.0_f64;
    let mut worst_drop = 0.0_f64;
    for point in gm.into_iter().filter(|point| point[1] >= 0.05 * maximum) {
        running = running.max(point[1]);
        worst_drop = worst_drop.max((running - point[1]) / running);
    }
    assert!(
        worst_drop < 0.02,
        "AOSTFT gm notch={:.1}%",
        100.0 * worst_drop
    );
}

#[test]
fn initial_level62_r2_describes_the_displayed_full_overlay() {
    let device = real_transfer_only();
    let view = device.model(FitModel::Level62);
    let expected_r2 = overlay_log_r2(&view.transfer_overlay(), &device.measured_points());
    let displayed_r2 = view.r2().expect("initial Level 62 fit");
    assert!(
        (displayed_r2 - expected_r2).abs() < 1.0e-12,
        "displayed R2={displayed_r2} overlay R2={expected_r2}"
    );
}

#[test]
fn level62_output_conductance_has_no_isolated_knee_spike() {
    let device = real_pair();
    let view = device.model(FitModel::Level62);
    let family = view.output_family();
    for curve in family.iter().filter(|curve| curve.vg <= -2.0) {
        let measured_start = nearest(&curve.measured, 8.0)[1];
        let measured_end = nearest(&curve.measured, 10.0)[1];
        let model_start = nearest(&curve.modelled, 8.0)[1];
        let model_end = nearest(&curve.modelled, 10.0)[1];
        let measured_rise = measured_end / measured_start - 1.0;
        let model_rise = model_end / model_start - 1.0;
        assert!(
            (model_rise - measured_rise).abs() < 0.02,
            "Vg={} measured tail rise={measured_rise:.3}, model={model_rise:.3}",
            curve.vg
        );
    }

    for OutputSeries {
        vg, mut modelled, ..
    } in view.gds_series()
    {
        modelled.sort_by(|a, b| a[0].total_cmp(&b[0]));
        for window in modelled.windows(3) {
            assert!(
                !(window[1][1] > window[0][1] && window[1][1] > window[2][1]),
                "Vg={vg}, Vd={} isolated gds spike: {:e} vs neighbors {:e}/{:e}",
                window[1][0],
                window[1][1],
                window[0][1],
                window[2][1]
            );
        }
    }
}

#[test]
fn level62_output_family_stays_close_to_the_real_pair() {
    let device = real_pair();
    let view = device.model(FitModel::Level62);
    assert!(
        view.r2().unwrap() > 0.982,
        "output refinement must preserve the transfer overlay"
    );
    let transfer = view.transfer_overlay();
    let family = view.output_family();
    let peak = family
        .iter()
        .flat_map(|curve| curve.measured.iter().map(|point| point[1]))
        .fold(0.0_f64, f64::max);
    let mut errors = Vec::new();
    for curve in &family {
        let transfer_at_gate = nearest(&transfer, curve.vg)[1];
        let output_at_bias = nearest(&curve.modelled, 5.0)[1];
        assert!(
            (transfer_at_gate - output_at_bias).abs() / transfer_at_gate < 1.0e-10,
            "Vg={} transfer={transfer_at_gate:e}, output={output_at_bias:e}",
            curve.vg
        );
        let curve_peak = curve
            .measured
            .iter()
            .map(|point| point[1])
            .fold(0.0_f64, f64::max);
        let curve_errors: Vec<_> = curve
            .measured
            .iter()
            .zip(&curve.modelled)
            .filter(|(measured, _)| measured[0] > 0.2)
            .map(|(measured, modelled)| (measured[1] - modelled[1]).powi(2))
            .collect();
        errors.extend(&curve_errors);
        let curve_nrmse =
            (curve_errors.iter().sum::<f64>() / curve_errors.len() as f64).sqrt() / curve_peak;
        if curve.vg <= -2.0 {
            assert!(
                curve_nrmse < 0.07,
                "Vg={} Level 62 output NRMSE={curve_nrmse:.3}",
                curve.vg
            );
        }
    }
    let nrmse = (errors.iter().sum::<f64>() / errors.len() as f64).sqrt() / peak;
    assert!(nrmse < 0.03, "Level 62 output NRMSE={nrmse:.3}");
}

#[test]
fn aostft_saturation_mapping_is_idempotent_and_detach_restores_the_h_fit() {
    let mut device = real_pair();
    let strict_gamma = device.aostft_fit().gamma;
    let strict_k = device.aostft_fit().k;
    let output = device.output().expect("output params");
    let bias = device.bias();
    device
        .set_geometry(device.geometry())
        .expect("geometry rebuild");
    assert!(
        (device.aostft_fit().gamma - strict_gamma).abs() < 1.0e-12,
        "a rebuild must not subtract the saturation exponent twice"
    );
    assert!(
        (device.aostft_fit().k - strict_k).abs() < 1.0e-18,
        "a rebuild must not rescale K twice"
    );

    device.detach_output().expect("attached output");
    let transfer_only = device.aostft_fit();
    assert!(
        (transfer_only.gamma - (strict_gamma + 1.0)).abs() < 1.0e-12,
        "detaching output must restore the original H-fit exponent"
    );
    let expected_h_k = strict_k * output.alpha_sat * (1.0 + output.lambda * bias.v_ds) / bias.v_ds;
    assert!(
        (transfer_only.k - expected_h_k).abs() < 1.0e-18,
        "detaching output must restore the original H-fit K"
    );
}

#[test]
fn aostft_manual_attach_and_detach_map_the_h_fit_once() {
    let mut device = real_transfer_only();
    let h_fit = *device.aostft_fit();
    device
        .set_aostft_fit(ModelParams {
            vt: h_fit.vt,
            gamma: h_fit.gamma,
            k: h_fit.k,
        })
        .expect("manual fit");

    let output = parse_output_file(&real_fixture("2-6o.xlsx")).expect("real output fixture");
    assert_eq!(
        device
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    let strict_fit = *device.aostft_fit();
    assert!((strict_fit.gamma - (h_fit.gamma - 1.0)).abs() < 1.0e-12);

    device.detach_output().expect("attached output");
    let restored = device.aostft_fit();
    assert!((restored.gamma - h_fit.gamma).abs() < 1.0e-12);
    assert!((restored.k - h_fit.k).abs() < 1.0e-18);
}

#[test]
fn aostft_manual_output_definition_remaps_through_the_original_h_fit() {
    let mut device = real_transfer_only();
    let h_fit = *device.aostft_fit();
    let first_output = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    device
        .set_aostft_output(first_output)
        .expect("manual output");
    let mapped = *device.aostft_fit();
    assert!((mapped.gamma - (h_fit.gamma - 1.0)).abs() < 1.0e-12);

    let second_output = OutputParams {
        alpha_sat: 0.8,
        lambda: 0.02,
        m: 3.0,
    };
    device
        .set_aostft_output(second_output)
        .expect("manual output edit");
    let edited = device.aostft_fit();
    assert!((edited.gamma - mapped.gamma).abs() < 1.0e-12);
    let bias = device.bias();
    let recovered_h_k =
        edited.k * second_output.alpha_sat * (1.0 + second_output.lambda * bias.v_ds) / bias.v_ds;
    assert!((recovered_h_k - h_fit.k).abs() < 1.0e-18);
    assert_ne!(
        edited.k, mapped.k,
        "changing the output definition must rescale the strict-card gain"
    );
}

#[test]
fn aostft_rejects_a_bias_that_would_overflow_the_manual_card_mapping() {
    let mut device = real_transfer_only();
    device
        .set_aostft_output(OutputParams {
            alpha_sat: 2.0,
            lambda: 1.0,
            m: 2.5,
        })
        .expect("finite at the transfer bias");
    let before = device.bias();
    let fit_before = *device.aostft_fit();

    assert_eq!(
        device.set_bias(f64::MAX, before.cox),
        Err(InputError::InvalidAostftCardMapping)
    );
    assert_eq!(device.bias(), before);
    assert_eq!(*device.aostft_fit(), fit_before);
}

#[test]
fn poor_aostft_extractions_keep_the_device_and_all_model_charts_finite() {
    for name in ["1-1.xlsx", "1227-3-3.xlsx"] {
        let device = folder_transfer(name);
        let view = device.model(FitModel::Aostft);
        assert!(
            !view.transfer_overlay().is_empty(),
            "{name} transfer model must remain visible"
        );
        assert!(
            view.transfer_overlay()
                .iter()
                .all(|point| point[0].is_finite() && point[1].is_finite()),
            "{name} transfer model must stay finite"
        );
        let gm = view.gm_series();
        assert!(!gm.is_empty(), "{name} gm must remain visible");
        assert!(
            gm.iter().all(|point| point[1].is_finite()),
            "{name} gm must stay finite"
        );
        assert!(
            !view.gds_series().is_empty(),
            "{name} gds must remain visible"
        );
        assert!(
            !view.gm_id_sizing_series().is_empty(),
            "{name} gm/Id sizing must remain visible"
        );
        assert!(
            !view.intrinsic_gain_series().is_empty(),
            "{name} intrinsic gain must remain visible"
        );
    }
}

#[test]
fn poor_level62_fit_stays_visible_instead_of_dropping_every_series() {
    let device = folder_transfer("7-2.xlsx");
    let view = device.model(FitModel::Level62);
    assert!(device.level62().is_some());
    assert!(!view.transfer_overlay().is_empty());
    assert!(!view.output_family().is_empty());
    assert!(!view.gm_series().is_empty());
    assert!(!view.gds_series().is_empty());
    assert!(!view.gm_id_sizing_series().is_empty());
    assert!(!view.intrinsic_gain_series().is_empty());
}

#[test]
fn folder_transfers_do_not_create_model_gm_notches() {
    let mut failures = Vec::new();
    for (name, model) in [
        ("7-2.xlsx", FitModel::Aostft),
        ("4-1.xlsx", FitModel::Level62),
        ("7-3.xlsx", FitModel::Level62),
        ("1227-3-5.xlsx", FitModel::Level62),
        ("3-52.xlsx", FitModel::Level62),
    ] {
        let device = folder_transfer(name);
        let view = device.model(model);
        let drop = worst_gm_drop(view.gm_series(), device.polarity());
        if drop >= 0.02 {
            failures.push(format!(
                "{name} model {model:?} gm drop={:.1}%",
                100.0 * drop
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn level62_tracks_the_real_3_52_gm_shape_for_analog_use() {
    let device = folder_transfer("3-52.xlsx");
    let view = device.model(FitModel::Level62);
    let error = view
        .analog_fit_quality()
        .gm_p90
        .expect("qualified gm metric");
    assert!(
        error < 0.15,
        "Level 62 analog gm P90 error={:.1}% despite transfer R2={:.6}",
        100.0 * error,
        view.r2().expect("model R2")
    );
}

#[test]
fn level62_real_2_6_gm_has_no_mid_on_shoulder() {
    let device = real_pair();
    let view = device.model(FitModel::Level62);
    let polarity = device.polarity();
    let normalize = |mut points: Vec<[f64; 2]>| {
        for point in &mut points {
            point[0] = polarity.map_vg(point[0]);
        }
        points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        points
    };
    let measured = normalize(device.measured_gm_series());
    let modelled = normalize(view.gm_series());
    let p95 = |points: &[[f64; 2]]| {
        let mut values: Vec<_> = points.iter().map(|point| point[1]).collect();
        values.sort_by(f64::total_cmp);
        values[((values.len() - 1) as f64 * 0.95).round() as usize]
    };
    let measured_peak = p95(&measured);
    let modelled_peak = p95(&modelled);
    let mut worst_retention = f64::INFINITY;
    for start in &measured {
        for end in &measured {
            let width = end[0] - start[0];
            let measured_start = start[1] / measured_peak;
            let measured_end = end[1] / measured_peak;
            let measured_rise = measured_end - measured_start;
            if (0.7..=0.9).contains(&width)
                && measured_start >= 0.25
                && measured_end <= 0.9
                && measured_rise >= 0.08
            {
                let modelled_rise = (nearest(&modelled, end[0])[1]
                    - nearest(&modelled, start[0])[1])
                    / modelled_peak;
                worst_retention = worst_retention.min(modelled_rise / measured_rise);
            }
        }
    }
    assert!(
        worst_retention.is_finite(),
        "fixture needs a mid-on gm window"
    );
    assert!(
        worst_retention >= 0.70,
        "Level 62 retains only {:.1}% of the measured local gm rise",
        100.0 * worst_retention
    );
    let gm_error = view
        .analog_fit_quality()
        .gm_p90
        .expect("qualified gm metric");
    assert!(
        gm_error < 0.15,
        "Level 62 analog gm P90 error={:.1}%",
        100.0 * gm_error
    );
}

#[test]
fn both_models_track_the_real_3_52_saturation_gds_for_analog_use() {
    let mut device = folder_transfer("3-52.xlsx");
    let output = parse_output_file(&real_fixture("3-52o.xlsx")).expect("real output fixture");
    assert_eq!(
        device
            .replace_output(output)
            .expect("output replacement preserves DIBL")
            .outcome,
        OutputAttachOutcome::Fitted
    );
    for (model, label) in [
        (FitModel::Aostft, "AOSTFT"),
        (FitModel::Level62, "Level 62"),
    ] {
        let view = device.model(model);
        let error = view
            .analog_fit_quality()
            .gds_p90
            .expect("qualified gds metric");
        assert!(
            error < 0.25,
            "{label} saturation-tail gds P90 error={:.1}%",
            100.0 * error
        );
    }
}

#[test]
fn full_overlay_r2_and_analog_qualification_expose_a_bad_fit() {
    let device = folder_transfer("4-1.xlsx");
    let view = device.model(FitModel::Level62);
    let r2 = view.r2().expect("Level 62 R2");
    let gm_error = view
        .analog_fit_quality()
        .gm_p90
        .expect("qualified gm metric");
    assert!(
        r2 < 0.9,
        "full-overlay R2 must expose the fixture's poor global fit: {r2}"
    );
    assert!(
        gm_error >= 0.15,
        "analog qualification must expose the gm mismatch: {:.1}%",
        100.0 * gm_error
    );
}
