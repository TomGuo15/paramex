use super::{cumulative_trapezoid, extract_above_threshold};
use crate::modelfit::forward::transfer_curve;
use crate::modelfit::ModelParams;

// Round-trip: synthesize a transfer curve with KNOWN parameters via the forward
// model, then confirm the UMEM H-function extraction recovers them. This is the
// self-consistency guard at the core of the forward-model-first strategy.

// A physical transfer sweep spans the below-threshold (off) region, so the
// H-function integral starts where Id is ~0.
fn sweep(start: f64, step: f64, last: f64) -> Vec<f64> {
    let n = ((last - start) / step).round() as usize;
    (0..=n).map(|i| start + i as f64 * step).collect()
}

#[test]
fn h_function_recovers_vt_gamma_k_from_synthetic_curve() {
    let truth = ModelParams {
        vt: 2.0,
        gamma: 0.5,
        k: 1e-6,
    };
    let vgs = sweep(0.0, 0.05, 20.0); // starts below VT=2.0
    let id = transfer_curve(&truth, &vgs);

    let got = extract_above_threshold(&vgs, &id).expect("extraction succeeds");

    assert!(
        (got.vt - truth.vt).abs() < 0.01,
        "vt={} (truth 2.0)",
        got.vt
    );
    assert!(
        (got.gamma - truth.gamma).abs() < 0.01,
        "gamma={} (truth 0.5)",
        got.gamma
    );
    let k_rel = (got.k - truth.k).abs() / truth.k;
    assert!(k_rel < 0.01, "k={} rel_err={k_rel} (truth 1e-6)", got.k);
}

#[test]
fn h_function_recovers_a_second_distinct_device() {
    // A different operating point (negative-ish VT, larger gamma) to prove the
    // recovery is not tuned to one case.
    let truth = ModelParams {
        vt: -1.5,
        gamma: 1.2,
        k: 4.0e-7,
    };
    let vgs = sweep(-5.0, 0.05, 13.0); // starts below VT=-1.5
    let id = transfer_curve(&truth, &vgs);

    let got = extract_above_threshold(&vgs, &id).expect("extraction succeeds");

    assert!(
        (got.vt - truth.vt).abs() < 0.02,
        "vt={} (truth -1.5)",
        got.vt
    );
    assert!(
        (got.gamma - truth.gamma).abs() < 0.02,
        "gamma={} (truth 1.2)",
        got.gamma
    );
    let k_rel = (got.k - truth.k).abs() / truth.k;
    assert!(k_rel < 0.02, "k={} rel_err={k_rel}", got.k);
}

// Deterministic LCG so the "noise" is fixed-seed reproducible (no rand dep).
fn lcg(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64) / ((1u64 << 53) as f64) // [0, 1)
}

#[test]
fn h_function_is_robust_to_measurement_noise() {
    let truth = ModelParams {
        vt: 1.0,
        gamma: 0.7,
        k: 2.0e-6,
    };
    let vgs = sweep(-2.0, 0.05, 20.0);
    let mut id = transfer_curve(&truth, &vgs);

    // 3% multiplicative noise on the conducting points; the integral denoises.
    let mut seed = 0x1234_5678_9abc_def0_u64;
    for v in id.iter_mut() {
        if *v > 0.0 {
            *v *= 1.0 + 0.03 * (2.0 * lcg(&mut seed) - 1.0);
        }
    }

    let got = extract_above_threshold(&vgs, &id).expect("extraction succeeds");

    assert!(
        (got.vt - truth.vt).abs() < 0.05,
        "vt={} (truth 1.0)",
        got.vt
    );
    assert!(
        (got.gamma - truth.gamma).abs() < 0.05,
        "gamma={} (truth 0.7)",
        got.gamma
    );
    let k_rel = (got.k - truth.k).abs() / truth.k;
    assert!(k_rel < 0.05, "k={} rel_err={k_rel}", got.k);
}

#[test]
fn extraction_returns_none_without_an_above_threshold_region() {
    let vgs = sweep(0.0, 0.1, 5.0);
    let id = vec![0.0; vgs.len()]; // no conduction at all
    assert!(extract_above_threshold(&vgs, &id).is_none());
}

#[test]
fn cumulative_trapezoid_matches_closed_forms_and_degenerate_inputs() {
    let line = cumulative_trapezoid(&[0.0, 1.0, 2.0, 3.0], &[0.0, 1.0, 2.0, 3.0]);
    assert_eq!(line, [0.0, 0.5, 2.0, 4.5]);

    let nonuniform = cumulative_trapezoid(&[0.0, 0.5, 2.0, 5.0], &[2.0, 2.0, 2.0, 2.0]);
    assert_eq!(nonuniform, [0.0, 1.0, 4.0, 10.0]);

    assert!(cumulative_trapezoid(&[], &[]).is_empty());
    assert_eq!(cumulative_trapezoid(&[1.0], &[5.0]), [0.0]);
}
