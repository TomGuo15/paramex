use super::*;

/// Round-trip: transfers at two drain biases synthesized from a known DIBL
/// strength let `refine_level62_dibl` recover `AT` (and keep `VTO` honest), starting
/// from a DIBL-off parameter set — the state every base extraction hands over.
#[test]
fn level62_dibl_refit_recovers_at_from_dual_vds_transfers() {
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let truth = Level62Params {
        vto: 1.0,
        at: 3.0e-8,
        eta: 5.0,
        mu0: 80.0e-4,
        mu1: 0.03e-4,
        mmu: 2.2,
        mus: 3.0e-4,
        ..Level62Params::ltps()
    };
    let vg: Vec<f64> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
    let low = level62_transfer(&truth, geom, T_NOM_K, &vg, 1.0);
    let high = level62_transfer(&truth, geom, T_NOM_K, &vg, 40.0);

    // The base fit hands over the truth with DIBL off (its VTO absorbed the shift
    // at the primary bias; keep it slightly off to prove VTO is co-freed).
    let base = Level62Params {
        at: 0.0,
        vto: 0.9,
        ..truth
    };
    let transfers = [
        (vg.as_slice(), low.as_slice(), 1.0),
        (vg.as_slice(), high.as_slice(), 40.0),
    ];
    let fit = refine_level62_dibl(base, &transfers, geom, T_NOM_K, Polarity::NChannel)
        .expect("DIBL refinement succeeds on a clean synthetic pair");
    assert!(
        (0.3e-8..=9.0e-8).contains(&fit.at),
        "AT recovered near 3e-8: {:e}",
        fit.at
    );
    assert!(
        (fit.vto - truth.vto).abs() < 0.3,
        "VTO stays honest: {} vs {}",
        fit.vto,
        truth.vto
    );
}

/// A p-channel pair (device-frame negative biases and gate sweeps) refines the same
/// way — the polarity fold matches the output-refinement convention.
#[test]
fn level62_dibl_refit_handles_pchannel_pair() {
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let truth = Level62Params {
        vto: 1.0,
        at: 3.0e-8,
        eta: 5.0,
        mu0: 80.0e-4,
        mu1: 0.03e-4,
        mmu: 2.2,
        mus: 3.0e-4,
        ..Level62Params::ltps()
    };
    let vg_on: Vec<f64> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
    let low = level62_transfer(&truth, geom, T_NOM_K, &vg_on, 1.0);
    let high = level62_transfer(&truth, geom, T_NOM_K, &vg_on, 40.0);
    let vg_dev: Vec<f64> = vg_on.iter().map(|&v| -v).collect();

    let base = Level62Params { at: 0.0, ..truth };
    let transfers = [
        (vg_dev.as_slice(), low.as_slice(), -1.0),
        (vg_dev.as_slice(), high.as_slice(), -40.0),
    ];
    let fit = refine_level62_dibl(base, &transfers, geom, T_NOM_K, Polarity::PChannel)
        .expect("p-channel DIBL refinement succeeds");
    assert!(
        (0.3e-8..=9.0e-8).contains(&fit.at),
        "AT recovered near 3e-8: {:e}",
        fit.at
    );
}

/// Two sweeps at (nearly) the same bias carry no DIBL information — the refinement
/// declines rather than inventing a value.
#[test]
fn level62_dibl_refit_declines_near_equal_biases() {
    let geom = GeometryParams {
        w_um: 100.0,
        l_um: 10.0,
    };
    let p = Level62Params {
        vto: 1.0,
        ..Level62Params::ltps()
    };
    let vg: Vec<f64> = (0..=120).map(|i| -4.0 + 0.1 * i as f64).collect();
    let a = level62_transfer(&p, geom, T_NOM_K, &vg, 1.0);
    let b = level62_transfer(&p, geom, T_NOM_K, &vg, 1.1);
    let transfers = [
        (vg.as_slice(), a.as_slice(), 1.0),
        (vg.as_slice(), b.as_slice(), 1.1),
    ];
    assert!(
        refine_level62_dibl(p, &transfers, geom, T_NOM_K, Polarity::NChannel).is_none(),
        "near-equal biases must decline"
    );
}
