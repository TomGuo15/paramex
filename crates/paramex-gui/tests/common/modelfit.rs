use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use egui_kittest::Harness;
use paramex_core::modelfit::{parse_output_file, FittedDevice, ModelParams, SubthresholdParams};
use paramex_core::transfer::parse_transfer_file;
use paramex_gui::app::ParamExApp;
use paramex_gui::workspaces::modelfit::state::{
    DeviceInstallOutcome, ModelFitState, OutputSource, PrimaryTransferSource,
};

type FixtureKey = (String, Option<String>);

const VGTE_SMOOTH_V: f64 = 5.0e-2;

fn fitted_cache() -> &'static Mutex<HashMap<FixtureKey, FittedDevice>> {
    static CACHE: OnceLock<Mutex<HashMap<FixtureKey, FittedDevice>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn demo_devices() -> Vec<FittedDevice> {
    static DEVICES: OnceLock<Mutex<Vec<FittedDevice>>> = OnceLock::new();

    DEVICES
        .get_or_init(|| {
            let transfer =
                parse_transfer_file(&fixture_path("2-6.xlsx")).expect("organic fixture parses");
            let mut organic = fit_device("demo: organic", transfer.vg, transfer.id_abs);
            assert!(organic
                .replace_output(
                    parse_output_file(&fixture_path("2-6o.xlsx")).expect("output fixture parses"),
                )
                .expect("device without retained DIBL accepts output")
                .displaced
                .is_empty());

            let transfer =
                parse_transfer_file(&fixture_path("7-3.xlsx")).expect("LTPS fixture parses");
            let ltps = fit_device("demo: LTPS", transfer.vg, transfer.id_abs);

            Mutex::new(vec![organic, ltps])
        })
        .lock()
        .expect("demo fixture cache lock remains available")
        .clone()
}

pub fn synthetic_transfer(params: &ModelParams, vgs: &[f64]) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| {
            let overdrive = vg - params.vt;
            if overdrive > 0.0 {
                params.k * overdrive.powf(1.0 + params.gamma)
            } else {
                0.0
            }
        })
        .collect()
}

pub fn synthetic_unified_transfer(
    vt: f64,
    gamma: f64,
    k: f64,
    subthreshold: &SubthresholdParams,
    vgs: &[f64],
) -> Vec<f64> {
    let swing = subthreshold.ss_v_dec.max(f64::MIN_POSITIVE);
    let leakage = subthreshold.ioff.max(0.0);
    let (blend_offset, blend_slope) = blend_params(swing, 1.0 + gamma);
    let above_threshold = |overdrive: f64| k * smooth_overdrive(overdrive).powf(1.0 + gamma);
    let anchor =
        above_threshold(blend_offset) * (-std::f64::consts::LN_10 * blend_offset / swing).exp();

    vgs.iter()
        .map(|&vg| {
            let overdrive = vg - vt;
            let weight = (overdrive - blend_offset) * blend_slope;
            let above = above_threshold(overdrive) / (1.0 + (-2.0 * weight).exp());
            let softplus = (2.0 * weight).max(0.0) + (1.0 + (-(2.0 * weight).abs()).exp()).ln();
            let below = anchor * (std::f64::consts::LN_10 * overdrive / swing - softplus).exp();
            above + below + leakage
        })
        .collect()
}

fn smooth_overdrive(overdrive: f64) -> f64 {
    0.5 * (overdrive + (overdrive * overdrive + VGTE_SMOOTH_V * VGTE_SMOOTH_V).sqrt())
}

fn blend_params(swing: f64, power: f64) -> (f64, f64) {
    let radius = power.max(0.0) * swing / std::f64::consts::LN_10;
    let offset = (radius * radius - VGTE_SMOOTH_V * VGTE_SMOOTH_V)
        .max(0.0)
        .sqrt();
    (offset, 2.0 / swing)
}

pub fn fixture_path(name: &str) -> PathBuf {
    super::crate_file("../paramex-core/tests/fixtures/modelfit").join(name)
}

pub fn fit_device(name: impl Into<String>, vg: Vec<f64>, id: Vec<f64>) -> FittedDevice {
    FittedDevice::fit(name.into(), vg, id).expect("GUI fixture must be fit-ready")
}

pub fn fitted_device(transfer_name: &str) -> FittedDevice {
    cached_fitted_device(transfer_name, None)
}

pub fn fitted_device_with_output(transfer_name: &str, output_name: &str) -> FittedDevice {
    cached_fitted_device(transfer_name, Some(output_name))
}

fn cached_fitted_device(transfer_name: &str, output_name: Option<&str>) -> FittedDevice {
    let key = (transfer_name.to_owned(), output_name.map(ToOwned::to_owned));
    let mut cache = fitted_cache().lock().expect("Model Fit fixture cache");
    if let Some(device) = cache.get(&key) {
        return device.clone();
    }

    let transfer =
        parse_transfer_file(&fixture_path(transfer_name)).expect("transfer fixture parses");
    let mut device = fit_device(transfer.name, transfer.vg, transfer.id_abs);
    if let Some(output_name) = output_name {
        let output = parse_output_file(&fixture_path(output_name)).expect("output fixture parses");
        assert!(device
            .replace_output(output)
            .expect("device without retained DIBL accepts output")
            .displaced
            .is_empty());
    }
    cache.insert(key, device.clone());
    device
}

pub fn install_device(state: &mut ModelFitState, device: FittedDevice) {
    install_device_at(state, device, None);
}

fn install_device_at(
    state: &mut ModelFitState,
    device: FittedDevice,
    source_path: Option<PathBuf>,
) {
    let transfer_name = device.name().to_owned();
    let output_source = device.has_output_curves().then(|| {
        OutputSource::new(format!("{}_output.xlsx", device.name()), None)
            .expect("fixture output source is visibly named")
    });
    assert_eq!(
        state
            .install_fitted_device(
                device,
                PrimaryTransferSource::new(transfer_name, source_path)
                    .expect("fixture primary source is visibly named"),
                output_source,
            )
            .expect("fixture device and sources agree"),
        DeviceInstallOutcome::Installed
    );
}

pub fn demo_state() -> ModelFitState {
    let mut state = ModelFitState::default();
    for device in demo_devices() {
        install_device(&mut state, device);
    }
    state
}

pub fn demo_state_with(name: &str, mutate: impl FnOnce(&mut FittedDevice)) -> ModelFitState {
    let baseline = demo_state();
    let mut devices = baseline
        .devices()
        .iter()
        .map(|entry| entry.device().clone())
        .collect::<Vec<_>>();
    let device = devices
        .iter_mut()
        .find(|device| device.name() == name)
        .unwrap_or_else(|| panic!("demo device {name} exists"));
    mutate(device);

    let mut state = ModelFitState::default();
    for device in devices {
        install_device(&mut state, device);
    }
    state
}

pub fn install_fixture(state: &mut ModelFitState, transfer_name: &str) {
    install_device_at(
        state,
        fitted_device(transfer_name),
        Some(fixture_path(transfer_name)),
    );
}

pub fn install_fixture_with_output(
    state: &mut ModelFitState,
    transfer_name: &str,
    output_name: &str,
) {
    assert_eq!(
        state
            .install_fitted_device(
                fitted_device_with_output(transfer_name, output_name),
                PrimaryTransferSource::new(transfer_name, Some(fixture_path(transfer_name)))
                    .expect("fixture primary source is visibly named"),
                Some(
                    OutputSource::new(output_name, Some(fixture_path(output_name)))
                        .expect("fixture output source is visibly named"),
                ),
            )
            .expect("fixture output curves and sources agree"),
        DeviceInstallOutcome::Installed
    );
}

pub fn run_until_model_worker_finishes(harness: &mut Harness<'_, ParamExApp>) {
    harness.step();
    for _ in 0..4_000 {
        if harness.state().modelfit_workspace().is_idle() {
            harness.step();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        harness.step();
    }
    panic!("Model Fit worker did not finish");
}
