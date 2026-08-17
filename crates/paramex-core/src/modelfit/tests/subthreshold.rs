use super::{extract_above_threshold, extract_subthreshold};
use crate::modelfit::forward::unified_transfer;
use crate::modelfit::SubthresholdParams;

// The unified transfer model adds a real off state: an exponential subthreshold
// region (slope = the subthreshold swing S) plus a leakage floor Ioff, blended
// smoothly into the above-threshold power law (AOSTFT Eq.28/29). These tests pin
// the shape and the round-trip recovery of S and Ioff, and confirm the
// above-threshold H-function still recovers VT/gamma on the fuller curve.

fn sweep(start: f64, step: f64, last: f64) -> Vec<f64> {
    let n = ((last - start) / step).round() as usize;
    (0..=n).map(|i| start + i as f64 * step).collect()
}

#[test]
fn unified_transfer_has_off_floor_subthreshold_and_on_region() {
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let vgs = sweep(-5.0, 0.05, 15.0); // VT = 2.0
    let id = unified_transfer(2.0, 0.5, 1.0e-6, &sub, &vgs);

    // Deep off (Vg = -5): the exponential subthreshold has decayed to ~0 (a real
    // off state on a log plot), not a hard zero.
    let off = id[0];
    assert!(
        off > 0.0 && off < 1.0e-10,
        "deep-off is tiny but positive, got {off}"
    );
    // On (Vg = 15): power-law, far above the floor.
    let on = *id.last().unwrap();
    assert!(on > 1.0e-6, "on current, got {on}");
    // Monotonic non-decreasing across the sweep.
    assert!(
        id.windows(2).all(|w| w[1] >= w[0] - 1e-18),
        "monotonic turn-on"
    );
}

#[test]
fn extract_subthreshold_round_trips_ss_and_ioff() {
    let truth = SubthresholdParams {
        ss_v_dec: 0.25,
        ioff: 5.0e-13,
    };
    let vgs = sweep(-6.0, 0.02, 16.0);
    let id = unified_transfer(2.0, 0.5, 1.0e-6, &truth, &vgs);

    let got = extract_subthreshold(&vgs, &id).expect("subthreshold fit");
    // The steepest-decade SS lands just inside the model's sigmoid-steepened
    // crossover (VT+DV), so the round-trip reads a few % LOW (~0.234 vs 0.25 here)
    // rather than dead-on. That bias is a property of the synthetic blend, not the
    // extractor (real curves have no such crossover). Bound it tightly AND assert
    // the direction, so any future drift in the blend shape is caught.
    assert!(
        got.ss_v_dec > 0.22 && got.ss_v_dec <= truth.ss_v_dec + 1.0e-9,
        "SS={} V/dec (truth 0.25; expected a few % low, in (0.22, 0.25])",
        got.ss_v_dec
    );
    assert!(
        got.ioff > 0.0 && got.ioff < 5.0e-12,
        "Ioff={} (truth 5e-13)",
        got.ioff
    );
}

#[test]
fn h_function_still_recovers_vt_gamma_on_unified_curve() {
    let sub = SubthresholdParams {
        ss_v_dec: 0.3,
        ioff: 1.0e-12,
    };
    let vgs = sweep(-5.0, 0.05, 18.0);
    let id = unified_transfer(2.0, 0.5, 1.0e-6, &sub, &vgs);

    let fit = extract_above_threshold(&vgs, &id).expect("above-threshold fit");
    assert!((fit.vt - 2.0).abs() < 0.05, "vt={}", fit.vt);
    assert!((fit.gamma - 0.5).abs() < 0.05, "gamma={}", fit.gamma);
}
