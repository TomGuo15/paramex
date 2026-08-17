use crate::transfer::test_support::{assert_close, f64_vec, load_reference_in, opt_win, parse_f64};
use crate::transfer::types::{ExtractionContext, SweepData, SweepExtractionResult};
use serde_json::Value;

fn assert_sweep(got: &SweepExtractionResult, exp: &Value, label: &str) {
    assert_close(got.vt, parse_f64(&exp["vt"]), 1e-9, 1e-12);
    assert_close(got.mobility, parse_f64(&exp["mobility"]), 1e-9, 1e-12);
    assert_close(got.ss_mv_dec, parse_f64(&exp["ss_mv_dec"]), 1e-9, 1e-12);
    assert_close(got.ion, parse_f64(&exp["ion"]), 1e-9, 1e-12);
    assert_close(got.ioff, parse_f64(&exp["ioff"]), 1e-9, 1e-12);
    assert_close(
        got.on_off_ratio,
        parse_f64(&exp["on_off_ratio"]),
        1e-9,
        1e-12,
    );
    let _ = label;
}

#[test]
fn extract_pipeline_matches_python() {
    let g = load_reference_in("extract", "extract_pipeline");
    for case in g["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let vg = f64_vec(&case["vg"]);
        let f_id = f64_vec(&case["f_id"]);
        let b_id = f64_vec(&case["b_id"]);
        let ctx = ExtractionContext {
            cox_f_per_cm2: parse_f64(&case["cox"]),
            aspect_ratio: parse_f64(&case["aspect"]),
        };
        let vt_range = opt_win(&case["vt_range"]);
        let ss_range = opt_win(&case["ss_range"]);
        let min_r2 = parse_f64(&case["min_r2"]);

        let fwd = SweepData {
            vg: vg.clone(),
            id_abs: f_id.clone(),
        };
        let bwd = SweepData {
            vg: vg.clone(),
            id_abs: b_id.clone(),
        };

        let single = super::sweep::extract_single_sweep(&fwd, ctx, vt_range, ss_range, min_r2);
        assert_sweep(&single, &case["single"], label);

        let forward = super::sweep::extract_single_sweep(&fwd, ctx, vt_range, ss_range, 0.98);
        let backward = super::sweep::extract_single_sweep(&bwd, ctx, vt_range, ss_range, 0.98);
        assert_sweep(&forward, &case["dual_forward"], label);
        assert_sweep(&backward, &case["dual_backward"], label);
    }
}
