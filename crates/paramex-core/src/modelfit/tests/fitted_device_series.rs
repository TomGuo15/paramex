// Render-ready scientific projections exposed by `FittedDevice`.

use super::{clone_fixture, FixtureCache};
use crate::modelfit::forward::{output_card_current, output_curve, transfer_curve};
use crate::modelfit::{
    FitModel, FittedDevice, ModelParams, OutputCurve, OutputParams, SubthresholdParams,
};

fn p_channel_device() -> FittedDevice {
    static FIXTURE: FixtureCache<FittedDevice> = FixtureCache::new();

    clone_fixture(&FIXTURE, || {
        let params = ModelParams {
            vt: 2.0,
            gamma: 0.5,
            k: 1.0e-6,
        };
        let vg: Vec<_> = (0..=140).map(|i| 2.0 - 0.1 * i as f64).collect();
        let normalized: Vec<_> = vg.iter().map(|v| -*v).collect();
        let id = transfer_curve(&params, &normalized);
        FittedDevice::fit("p-channel".into(), vg, id).expect("p-channel fits")
    })
}

fn output_family() -> Vec<OutputCurve> {
    let output = OutputParams {
        alpha_sat: 0.7,
        lambda: 0.01,
        m: 2.5,
    };
    let vds: Vec<_> = (0..=150).map(|i| i as f64 * 0.1).collect();
    [2.0, 4.0, 6.0, 8.0]
        .into_iter()
        .map(|overdrive| OutputCurve {
            vg: -(2.0 + overdrive),
            vds: vds.iter().map(|v| -*v).collect(),
            id: output_curve(2.0, &output, 1.0e-5, 2.0 + overdrive, &vds),
        })
        .collect()
}

#[test]
fn p_channel_projection_families_are_finite_and_use_the_device_frame() {
    let mut device = p_channel_device();
    assert_eq!(device.polarity().sign(), -1.0);
    assert!(device
        .replace_output(output_family())
        .expect("output replacement preserves DIBL")
        .displaced
        .is_empty());

    for model in [FitModel::Aostft, FitModel::Level62] {
        let view = device.model(model);
        let transfer = view.transfer_overlay();
        assert_eq!(transfer.len(), device.measured_points().len());
        assert!(transfer
            .iter()
            .all(|p| p[0].is_finite() && p[1].is_finite()));
        assert!(
            transfer.iter().any(|p| p[0] < 0.0),
            "transfer remains in the p-channel device frame"
        );

        let output = view.output_family();
        assert!(!output.is_empty());
        assert!(output.iter().all(|series| {
            series
                .measured
                .iter()
                .chain(&series.modelled)
                .all(|p| p[0].is_finite() && p[1].is_finite() && p[0] >= 0.0 && p[1] >= 0.0)
        }));

        let gm = view.gm_series();
        let gds = view.gds_series();
        let sizing = view.gm_id_sizing_series();
        let gain = view.intrinsic_gain_series();
        assert_eq!(gm.len(), 200);
        assert!(!gds.is_empty());
        assert!(!sizing.is_empty());
        assert!(!gain.is_empty());
        assert!(gm.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
        assert!(gds
            .iter()
            .flat_map(|s| s.measured.iter().chain(&s.modelled))
            .all(|p| { p[0].is_finite() && p[1].is_finite() }));
        assert!(sizing.iter().all(|p| p[0] > 0.0 && p[1] > 0.0));
        assert!(gain
            .iter()
            .all(|p| p[0] > 0.0 && p[1].is_finite() && p[1] > 0.0));
        assert!(gain
            .windows(2)
            .all(|w| w[1][0] > w[0][0] && w[1][1] >= w[0][1]));
    }
}

#[test]
fn model_view_quality_and_export_follow_the_current_manual_parameters() {
    let mut device = p_channel_device();
    let mut params = *device.aostft_fit();
    params.vt -= 1.25;
    device
        .set_aostft_fit(ModelParams {
            vt: params.vt,
            gamma: params.gamma,
            k: params.k,
        })
        .expect("manual fit commits");
    let view = device.model(FitModel::Aostft);
    assert!(view.is_manual());
    assert!(view.r2().is_some_and(f64::is_finite));
    assert!(view.analog_fit_quality().gm_p90.is_some());
    let artifact = view.export_artifact().expect("AOSTFT always exports");
    assert_eq!(artifact.suggested_file_name, "p_channel.va");
    assert!(
        artifact.text.contains("ahdl_include \"p_channel.va\""),
        "the suggested filename and the include example must share core sanitization"
    );
    let card = artifact.text;
    assert!(
        card.contains(&format!("parameter real VTO = {:.4e}", params.vt)),
        "the card must use the current manual VTO"
    );
}

#[test]
fn transfer_only_aostft_uses_one_strict_card_projection_for_output_gain_and_export() {
    let mut device = p_channel_device();
    let extracted = *device.aostft_fit();
    device
        .set_aostft_fit(ModelParams {
            vt: extracted.vt,
            gamma: 0.0,
            k: extracted.k,
        })
        .expect("zero H mobility exponent is circuit-usable");
    let h_fit = *device.aostft_fit();
    let transfer_before = device.model(FitModel::Aostft).transfer_overlay();
    let bias = device.bias();
    let geometry = device.geometry();
    let output = OutputParams::card_defaults();
    let subthreshold = device
        .subthreshold()
        .unwrap_or_else(SubthresholdParams::card_defaults);
    let scale = output.alpha_sat * (1.0 + output.lambda * bias.v_ds);
    let card_gamma = h_fit.gamma - 1.0;
    let card_k = h_fit.k * (bias.v_ds / scale);

    let view = device.model(FitModel::Aostft);
    assert!(view.is_export_ready());
    let family = view.output_family();
    assert_eq!(family.len(), 4);
    let turn_on = device.polarity().map_vg(h_fit.vt);
    for series in family {
        let vd: Vec<_> = series.modelled.iter().map(|point| point[0]).collect();
        let vov = device.polarity().map_vg(series.vg) - turn_on;
        let expected = output_card_current(
            card_k / bias.v_ds,
            card_gamma,
            bias.r,
            &output,
            &subthreshold,
            vov,
            &vd,
        );
        assert_eq!(
            series
                .modelled
                .iter()
                .map(|point| point[1])
                .collect::<Vec<_>>(),
            expected,
            "the predicted family must exactly reproduce the projected strict card"
        );
    }
    assert!(
        !view.intrinsic_gain_series().is_empty(),
        "gain must use the same representable strict-card projection"
    );

    let card = view.export_artifact().expect("projection exports").text;
    let kp = geometry.per_square_kp(card_k / bias.v_ds);
    assert_eq!(card_gamma, -1.0);
    assert!(card.contains("parameter real GAMMA = -1.0000e0"));
    assert!(card.contains(&format!("parameter real KP = {kp:.4e}")));
    assert_eq!(
        *device.aostft_fit(),
        h_fit,
        "read projections must not rewrite the stored H fit"
    );
    assert_eq!(
        device.model(FitModel::Aostft).transfer_overlay(),
        transfer_before,
        "the transfer overlay remains in the H-function representation"
    );
}

#[test]
fn an_unrepresentable_h_fit_keeps_its_transfer_overlay_but_has_no_strict_card_views() {
    let mut device = p_channel_device();
    let fit = *device.aostft_fit();
    device
        .set_aostft_fit(ModelParams {
            vt: fit.vt,
            gamma: -0.5,
            k: fit.k,
        })
        .expect("the H transfer exponent remains physically valid");

    let view = device.model(FitModel::Aostft);
    assert!(!view.transfer_overlay().is_empty());
    assert!(!view.is_export_ready());
    assert!(view.output_family().is_empty());
    assert!(view.intrinsic_gain_series().is_empty());
    assert!(view.export_artifact().is_none());
}
