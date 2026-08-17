use super::transfer_curve;
use crate::modelfit::ModelParams;

// The AOSTFT above-threshold drain current as a function of gate voltage is a
// power law in the gate overdrive: Id = K * (Vg - VT)^(1 + gamma) for Vg > VT,
// and (modelled as) 0 below threshold. This is the exact shape the UMEM
// H-function inverts, so it is the foundation of the forward model.

#[test]
fn transfer_curve_is_power_law_above_threshold_zero_below() {
    let p = ModelParams {
        vt: 2.0,
        gamma: 0.5,
        k: 1e-6,
    };
    let vgs = [0.0, 1.0, 2.0, 3.0, 4.0];
    let id = transfer_curve(&p, &vgs);

    assert_eq!(id.len(), vgs.len());
    assert_eq!(id[0], 0.0, "below threshold -> 0");
    assert_eq!(id[2], 0.0, "at threshold (overdrive 0) -> 0");

    let expected3 = 1e-6 * (1.0_f64).powf(1.5);
    let expected4 = 1e-6 * (2.0_f64).powf(1.5);
    assert!(
        (id[3] - expected3).abs() <= 1e-18,
        "id[3]={} expected={expected3}",
        id[3]
    );
    assert!(
        (id[4] - expected4).abs() <= 1e-15,
        "id[4]={} expected={expected4}",
        id[4]
    );
}
