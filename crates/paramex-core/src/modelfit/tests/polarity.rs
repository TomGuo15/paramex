// Polarity + dual-sweep preparation: `prepare_transfer` must take one monotonic
// branch of a hysteresis sweep and detect p- vs n-channel, so the UMEM extractor
// recovers VT/gamma for BOTH polarities and for doubled-back sweeps (the shape of
// real measured data — e.g. a p-channel device swept 5 -> -10 -> 5 V).

use super::{extract_above_threshold, prepare_transfer};
use crate::modelfit::forward::unified_transfer;
use crate::modelfit::{Polarity, SubthresholdParams};

const SUB: SubthresholdParams = SubthresholdParams {
    ss_v_dec: 0.3,
    ioff: 1.0e-12,
};

/// An n-channel transfer (VT=2): Vg from -3 to 15, on at high Vg.
fn n_curve() -> (Vec<f64>, Vec<f64>) {
    let vgs: Vec<f64> = (0..=180).map(|i| -3.0 + i as f64 * 0.1).collect();
    let id = unified_transfer(2.0, 0.5, 1.0e-6, &SUB, &vgs);
    (vgs, id)
}

fn dual(vg: &[f64], id: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut dv = vg.to_vec();
    let mut di = id.to_vec();
    dv.extend(vg.iter().rev().copied());
    di.extend(id.iter().rev().copied());
    (dv, di)
}

#[test]
fn n_channel_single_sweep_is_unchanged_and_fits() {
    let (vg, id) = n_curve();
    let prepared = prepare_transfer(&vg, &id).expect("prepared");
    assert_eq!(prepared.polarity(), Polarity::NChannel);
    assert_eq!(prepared.vg(), vg);
    assert_eq!(prepared.id(), id);
    let fit = extract_above_threshold(prepared.vg(), prepared.id()).expect("fit");
    let vt = prepared.polarity().map_vg(fit.vt); // n-channel: identity
    assert!((vt - 2.0).abs() < 0.1, "VT={vt}");
    assert!((fit.gamma - 0.5).abs() < 0.1, "gamma={}", fit.gamma);
}

#[test]
fn n_channel_off_side_leakage_is_trimmed_before_the_h_fit() {
    let (vg, id) = n_curve();
    let mut with_leakage_vg = vec![-5.0, -4.0];
    let mut with_leakage_id = vec![1.0e-7, 1.0e-9];
    with_leakage_vg.extend_from_slice(&vg);
    with_leakage_id.extend_from_slice(&id);

    let prepared = prepare_transfer(&with_leakage_vg, &with_leakage_id).expect("prepared");

    assert_eq!(prepared.polarity(), Polarity::NChannel);
    assert_eq!(prepared.vg(), vg);
    assert_eq!(prepared.id(), id);
    let fit = extract_above_threshold(prepared.vg(), prepared.id()).expect("physical H fit");
    assert!((fit.vt - 2.0).abs() < 0.1, "VT={}", fit.vt);
}

#[test]
fn p_channel_single_sweep_is_detected_and_fits() {
    // A p-channel device with VT = -2: mirror the n-channel curve through Vg=0,
    // so it is on at NEGATIVE Vg.
    let (nvg, id) = n_curve();
    let pvg: Vec<f64> = nvg.iter().map(|v| -v).collect();
    let prepared = prepare_transfer(&pvg, &id).expect("prepared");
    assert_eq!(prepared.polarity(), Polarity::PChannel);
    let fit = extract_above_threshold(prepared.vg(), prepared.id()).expect("fit");
    let vt = prepared.polarity().map_vg(fit.vt); // p-channel: negate back to the device frame
    assert!((vt - (-2.0)).abs() < 0.1, "p-channel VT={vt} (expected -2)");
    assert!((fit.gamma - 0.5).abs() < 0.1, "gamma={}", fit.gamma);
}

#[test]
fn dual_sweep_takes_one_branch_and_fits_n_channel() {
    let (vg, id) = n_curve();
    let (dvg, did) = dual(&vg, &id); // -3..15..-3
    let prepared = prepare_transfer(&dvg, &did).expect("prepared");
    assert_eq!(prepared.polarity(), Polarity::NChannel);
    // One monotonic branch only (not the doubled length).
    assert!(
        prepared.vg().len() <= vg.len() + 1,
        "took one branch, got {}",
        prepared.vg().len()
    );
    let fit = extract_above_threshold(prepared.vg(), prepared.id()).expect("fit");
    assert!(
        (prepared.polarity().map_vg(fit.vt) - 2.0).abs() < 0.1,
        "VT={}",
        fit.vt
    );
}

#[test]
fn p_channel_dual_sweep_like_real_data_fits() {
    // The user's case: p-channel, swept high -> low -> high (e.g. 5 -> -10 -> 5).
    let (nvg, id) = n_curve();
    let pvg: Vec<f64> = nvg.iter().map(|v| -v).collect();
    let (dvg, did) = dual(&pvg, &id);
    let prepared = prepare_transfer(&dvg, &did).expect("prepared");
    assert_eq!(prepared.polarity(), Polarity::PChannel);
    assert!(
        prepared.vg().len() <= nvg.len() + 1,
        "one branch, got {}",
        prepared.vg().len()
    );
    let fit = extract_above_threshold(prepared.vg(), prepared.id()).expect("fit");
    let vt = prepared.polarity().map_vg(fit.vt);
    assert!(
        (vt - (-2.0)).abs() < 0.1,
        "p-channel dual VT={vt} (expected -2)"
    );
    assert!((fit.gamma - 0.5).abs() < 0.1, "gamma={}", fit.gamma);
    assert!(
        fit.r2 > 0.95,
        "a clean p-channel fit should have high R2, got {}",
        fit.r2
    );
}
