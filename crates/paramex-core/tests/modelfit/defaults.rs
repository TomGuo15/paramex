use paramex_core::modelfit::{BiasParams, GeometryParams};
use paramex_core::transfer::{DeviceGeometry, Session};

#[test]
fn shared_device_defaults_match_transfer_workspace() {
    let mf_geom = GeometryParams::default();
    let transfer_geom = DeviceGeometry::default();
    assert_eq!(mf_geom.w_um, transfer_geom.width_um);
    assert_eq!(mf_geom.l_um, transfer_geom.length_um);

    let mf_bias = BiasParams::default();
    let transfer_cox_f_m2 = Session::new().cox_nf_per_cm2() * 1.0e-5;
    assert_eq!(mf_bias.cox, transfer_cox_f_m2);
    assert_eq!(mf_bias.v_ds, 0.1);
    assert_eq!(mf_bias.r, 0.0);
}
