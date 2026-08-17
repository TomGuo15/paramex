use super::*;

/// Round-trip: synthesize a transfer from known Level 62 parameters, recover them with
/// `extract_level62` (data-driven seeds + multi-start LM on the overlay objective). The
/// well-identified parameters (VTO, ETA) come back; the overlay is near-perfect. The
/// (MU0, MU1, MMU) trio is intentionally NOT pinned tight — they trade off in the
/// reciprocal-sum µFET over a finite gate range, so the curve, not each value, is the
/// invariant.
#[test]
fn level62_round_trip_recovers_transfer_parameters() {
    let truth = Level62Params {
        vto: 1.2,
        eta: 5.0,
        mu0: 80.0e-4,
        mu1: 0.03e-4,
        mmu: 2.2,
        mus: 3.0e-4,
        ..Level62Params::ltps()
    };
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let v_ds = 0.1;
    let vg: Vec<f64> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
    let id = level62_transfer(&truth, geom, T_NOM_K, &vg, v_ds);

    let fit = extract_level62(&vg, &id, geom, v_ds, T_NOM_K, Level62Params::ltps())
        .expect("extraction succeeds on a clean synthetic curve");
    assert!(
        fit.r2 > 0.99,
        "near-perfect overlay on noiseless data: {}",
        fit.r2
    );
    assert!(
        (fit.params.vto - truth.vto).abs() < 0.25,
        "VTO recovered: {} vs {}",
        fit.params.vto,
        truth.vto
    );
    assert!(
        (fit.params.eta - truth.eta).abs() < 1.2,
        "ETA recovered: {} vs {}",
        fit.params.eta,
        truth.eta
    );
    assert_eq!(fit.polarity, Polarity::NChannel);
}

/// Under fixed-seed ±2 % multiplicative measurement noise the extraction degrades
/// gracefully: the overlay stays high and VTO stays close.
#[test]
fn level62_extraction_is_robust_to_noise() {
    let truth = Level62Params {
        vto: 1.2,
        eta: 5.0,
        mu0: 80.0e-4,
        mu1: 0.03e-4,
        mmu: 2.2,
        mus: 3.0e-4,
        ..Level62Params::ltps()
    };
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let v_ds = 0.1;
    let vg: Vec<f64> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
    let clean = level62_transfer(&truth, geom, T_NOM_K, &vg, v_ds);
    let mut seed: u64 = 0x1234_5678_9abc_def0;
    let id: Vec<f64> = clean
        .iter()
        .map(|&v| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let u = (seed >> 40) as f64 / (1u64 << 24) as f64;
            v * (1.0 + 0.02 * (u - 0.5))
        })
        .collect();

    let fit = extract_level62(&vg, &id, geom, v_ds, T_NOM_K, Level62Params::ltps())
        .expect("extraction succeeds on noisy data");
    assert!(
        fit.r2 > 0.97,
        "overlay stays high under ±2% noise: {}",
        fit.r2
    );
    assert!(
        (fit.params.vto - truth.vto).abs() < 0.4,
        "VTO stays close under noise: {}",
        fit.params.vto
    );
}

#[test]
fn level62_retains_a_finite_candidate_with_negative_r2() {
    let vg: Vec<f64> = (0..=20).map(|i| -5.0 + 0.5 * i as f64).collect();
    let id: Vec<f64> = (0..=20)
        .map(|i| if i % 2 == 0 { 1.0e-3 } else { 1.0e-2 })
        .collect();
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };

    let fit = extract_level62(&vg, &id, geom, 0.1, T_NOM_K, Level62Params::ltps())
        .expect("a poor finite fit must remain visible");

    assert!(fit.r2.is_finite());
    assert!(fit.r2 < 0.0, "fixture must remain a visibly poor fit");
}
