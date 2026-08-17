//! Transconductance-efficiency projection for Model Fit.

/// Transconductance efficiency `gm/Id` (V⁻¹), elementwise — the Silveira–Flandre–Jespers
/// design figure of merit. Uses the on-direction drain-current magnitude so the ratio is
/// the positive efficiency for n- and p-channel alike; the deep-off region (`|Id|` below a
/// tiny floor, where the ratio is meaningless and would blow up on noise) yields `0`.
pub(super) fn gm_over_id(gm: &[f64], id: &[f64]) -> Vec<f64> {
    const ID_FLOOR: f64 = 1.0e-15; // ~1 fA; below this the device is off and gm/Id is noise.
    gm.iter()
        .zip(id)
        .map(|(&g, &i)| {
            let mag = i.abs();
            if mag > ID_FLOOR {
                g / mag
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gm_over_id_is_positive_efficiency_and_floors_the_off_region() {
        // Subthreshold exponential: gm/Id = 1/(n·Vt) is constant where Id is meaningful.
        let id = [1.0e-9, 1.0e-8, 1.0e-7]; // a decade-per-step on-region
        let gm = [1.0e-9, 1.0e-8, 1.0e-7]; // gm = Id here ⇒ gm/Id = 1 V⁻¹
        let eff = gm_over_id(&gm, &id);
        for e in eff {
            assert!((e - 1.0).abs() < 1e-12, "gm/Id should be 1 V⁻¹, got {e}");
        }
        // p-channel frame (negative Id) uses the magnitude, still positive efficiency.
        assert!((gm_over_id(&[2.0e-9], &[-1.0e-9])[0] - 2.0).abs() < 1e-12);
        // Deep-off (sub-fA Id) floors to 0 instead of exploding on noise.
        assert_eq!(gm_over_id(&[1.0e-9], &[1.0e-18]), vec![0.0]);
    }
}
