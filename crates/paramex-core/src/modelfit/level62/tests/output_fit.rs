use super::*;

#[test]
fn level62_output_refit_recovers_output_dependent_terms() {
    let geom = unit_geom();
    let transfer_vds = 5.0;
    let base = Level62Params {
        vto: 1.0,
        asat: 1.2,
        lambda: 0.0,
        vkink: 30.0,
        ..Level62Params::ltps()
    };
    let truth = Level62Params {
        asat: 0.72,
        lambda: 0.025,
        vkink: 2.8,
        ..base
    };
    let base = transfer_calibrated_seed(base, truth, transfer_vds);
    let vds: Vec<f64> = (0..=80).map(|i| i as f64 * 0.15).collect();
    let curves: Vec<OutputCurve> = [2.4, 3.2, 4.0, 4.8]
        .into_iter()
        .map(|vg| OutputCurve {
            vg,
            vds: vds.clone(),
            id: level62_output(&truth, geom, T_NOM_K, vg, &vds),
        })
        .collect();
    let transfer_before = level62_current(&base, geom, T_NOM_K, 4.8, transfer_vds);

    let fit = refine_level62_output(
        base,
        &curves,
        geom,
        T_NOM_K,
        transfer_vds,
        (&[], &[]),
        Polarity::NChannel,
    )
    .expect("output refit");

    assert!((fit.asat - truth.asat).abs() < 0.08, "ASAT={}", fit.asat);
    assert!(
        (fit.lambda - truth.lambda).abs() < 0.01,
        "LAMBDA={}",
        fit.lambda
    );
    assert!((fit.vkink - truth.vkink).abs() < 1.0, "VKINK={}", fit.vkink);
    let transfer_after = level62_current(&fit, geom, T_NOM_K, 4.8, transfer_vds);
    assert!(
        (transfer_after - transfer_before).abs() / transfer_before < 0.01,
        "output refit changed transfer anchor: before={transfer_before:e}, after={transfer_after:e}"
    );
}

#[test]
fn level62_output_refit_does_not_invent_a_kink_for_flat_tails() {
    let geom = unit_geom();
    let transfer_vds = 5.0;
    let base = Level62Params {
        vto: 1.0,
        asat: 1.2,
        lambda: 0.0,
        vkink: 9.1,
        ..Level62Params::ltps()
    };
    let truth = Level62Params {
        asat: 0.72,
        lambda: 0.001,
        vkink: 1.0e6,
        ..base
    };
    let base = transfer_calibrated_seed(base, truth, transfer_vds);
    let vds: Vec<f64> = (0..=80).map(|i| i as f64 * 0.15).collect();
    let curves: Vec<OutputCurve> = [(2.4, 0.85), (3.2, 1.10), (4.0, 0.90), (4.8, 1.15)]
        .into_iter()
        .map(|(vg, scale)| OutputCurve {
            vg,
            vds: vds.clone(),
            id: level62_output(&truth, geom, T_NOM_K, vg, &vds)
                .into_iter()
                .map(|id| scale * id)
                .collect(),
        })
        .collect();

    let fit = refine_level62_output(
        base,
        &curves,
        geom,
        T_NOM_K,
        transfer_vds,
        (&[], &[]),
        Polarity::NChannel,
    )
    .expect("flat-tail output refit");
    assert!(
        fit.vkink >= 1.0e5,
        "gate-amplitude error must not be hidden by a fake kink: VKINK={}",
        fit.vkink
    );
}

#[test]
fn level62_output_refit_does_not_mistake_clm_for_a_kink() {
    let geom = unit_geom();
    let transfer_vds = 5.0;
    let base = Level62Params {
        vto: 1.0,
        asat: 1.2,
        lambda: 0.0,
        vkink: 9.1,
        ..Level62Params::ltps()
    };
    let truth = Level62Params {
        asat: 0.72,
        lambda: 0.03,
        vkink: 1.0e6,
        ..base
    };
    let base = transfer_calibrated_seed(base, truth, transfer_vds);
    let vds: Vec<f64> = (0..=80).map(|i| i as f64 * 0.15).collect();
    let curves: Vec<OutputCurve> = [(2.4, 0.90), (3.2, 1.08), (4.0, 0.94), (4.8, 1.06)]
        .into_iter()
        .map(|(vg, scale)| OutputCurve {
            vg,
            vds: vds.clone(),
            id: level62_output(&truth, geom, T_NOM_K, vg, &vds)
                .into_iter()
                .map(|id| scale * id)
                .collect(),
        })
        .collect();

    let fit = refine_level62_output(
        base,
        &curves,
        geom,
        T_NOM_K,
        transfer_vds,
        (&[], &[]),
        Polarity::NChannel,
    )
    .expect("CLM-only output refit");
    assert!(
        fit.vkink >= 1.0e5,
        "ordinary CLM must not enable a fake kink: VKINK={}",
        fit.vkink
    );
}

/// An over-sampled family (thousands of points per gate) must still recover the
/// output terms: the refit decimates to a flat sample cap before the O(samples)
/// LM (the transfer-path `FIT_POINT_CAP` cliff, reintroduced for outputs), and
/// the decimated family carries the same shape.
#[test]
fn level62_output_refit_survives_oversampled_family() {
    let geom = unit_geom();
    let transfer_vds = 5.0;
    let base = Level62Params {
        vto: 1.0,
        asat: 1.2,
        lambda: 0.0,
        vkink: 30.0,
        ..Level62Params::ltps()
    };
    let truth = Level62Params {
        asat: 0.72,
        lambda: 0.025,
        vkink: 2.8,
        ..base
    };
    let base = transfer_calibrated_seed(base, truth, transfer_vds);
    let vds: Vec<f64> = (0..=3000).map(|i| i as f64 * 0.004).collect();
    let curves: Vec<OutputCurve> = [2.4, 3.2, 4.0, 4.8]
        .into_iter()
        .map(|vg| OutputCurve {
            vg,
            vds: vds.clone(),
            id: level62_output(&truth, geom, T_NOM_K, vg, &vds),
        })
        .collect();

    let fit = refine_level62_output(
        base,
        &curves,
        geom,
        T_NOM_K,
        transfer_vds,
        (&[], &[]),
        Polarity::NChannel,
    )
    .expect("over-sampled output refit");

    assert!((fit.asat - truth.asat).abs() < 0.08, "ASAT={}", fit.asat);
    assert!(
        (fit.lambda - truth.lambda).abs() < 0.01,
        "LAMBDA={}",
        fit.lambda
    );
    assert!((fit.vkink - truth.vkink).abs() < 1.0, "VKINK={}", fit.vkink);
}

#[test]
fn level62_output_refit_handles_pchannel_curves() {
    let geom = unit_geom();
    let transfer_vds = 5.0;
    let base = Level62Params {
        vto: 1.0,
        asat: 1.2,
        lambda: 0.0,
        vkink: 30.0,
        ..Level62Params::ltps()
    };
    let truth = Level62Params {
        asat: 0.72,
        lambda: 0.025,
        vkink: 2.8,
        ..base
    };
    let base = transfer_calibrated_seed(base, truth, transfer_vds);
    let vds_on: Vec<f64> = (0..=80).map(|i| i as f64 * 0.15).collect();
    let vds_device: Vec<f64> = vds_on.iter().map(|&v| -v).collect();
    let curves: Vec<OutputCurve> = [2.4, 3.2, 4.0, 4.8]
        .into_iter()
        .map(|vg_on| OutputCurve {
            vg: -vg_on,
            vds: vds_device.clone(),
            id: level62_output(&truth, geom, T_NOM_K, vg_on, &vds_on),
        })
        .collect();

    let fit = refine_level62_output(
        base,
        &curves,
        geom,
        T_NOM_K,
        transfer_vds,
        (&[], &[]),
        Polarity::PChannel,
    )
    .expect("p-channel output refit");

    assert!((fit.asat - truth.asat).abs() < 0.08, "ASAT={}", fit.asat);
    assert!(
        (fit.lambda - truth.lambda).abs() < 0.01,
        "LAMBDA={}",
        fit.lambda
    );
    assert!((fit.vkink - truth.vkink).abs() < 1.0, "VKINK={}", fit.vkink);
}
