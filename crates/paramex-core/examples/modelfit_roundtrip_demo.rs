//! Demonstrates the UMEM H-function round-trip for the compact-model (Model Fit)
//! seam: synthesize TFT transfer curves with KNOWN AOSTFT above-threshold
//! parameters, then extract them back and print truth-vs-recovered. All data is
//! synthetic; this is a developer demo, not a production binary.
//!
//! Run: `cargo run -p paramex-core --example modelfit_roundtrip_demo`

use paramex_core::modelfit::{FittedDevice, ModelParams};

fn sweep(start: f64, step: f64, last: f64) -> Vec<f64> {
    let n = ((last - start) / step).round() as usize;
    (0..=n).map(|i| start + i as f64 * step).collect()
}

fn synthetic_transfer(params: &ModelParams, vgs: &[f64]) -> Vec<f64> {
    vgs.iter()
        .map(|&vg| params.k * (vg - params.vt).max(0.0).powf(1.0 + params.gamma))
        .collect()
}

fn main() {
    let devices = [
        (
            "organic-ish",
            ModelParams {
                vt: 2.0,
                gamma: 0.5,
                k: 1.0e-6,
            },
        ),
        (
            "oxide-ish",
            ModelParams {
                vt: -1.5,
                gamma: 1.2,
                k: 4.0e-7,
            },
        ),
        (
            "a-Si-ish",
            ModelParams {
                vt: 4.0,
                gamma: 0.35,
                k: 8.0e-7,
            },
        ),
    ];

    println!("UMEM H-function round-trip on synthetic AOSTFT transfer curves\n");
    println!(
        "{:<12} {:>8} {:>8}   {:>7} {:>7}   {:>11} {:>11}   {:>8}",
        "device", "VT_true", "VT_fit", "g_true", "g_fit", "K_true", "K_fit", "fit R2"
    );
    println!("{}", "-".repeat(82));
    for (name, truth) in devices {
        let vgs = sweep(truth.vt - 4.0, 0.05, truth.vt + 16.0);
        let id = synthetic_transfer(&truth, &vgs);
        let device =
            FittedDevice::fit(name.to_string(), vgs, id).expect("above-threshold region present");
        let fit = device.aostft_fit();
        println!(
            "{:<12} {:>8.3} {:>8.3}   {:>7.3} {:>7.3}   {:>11.3e} {:>11.3e}   {:>8.6}",
            name, truth.vt, fit.vt, truth.gamma, fit.gamma, truth.k, fit.k, fit.r2
        );
    }
}
