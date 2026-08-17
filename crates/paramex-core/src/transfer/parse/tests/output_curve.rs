use crate::transfer::parse::{looks_like_output_curve, Grid, OUTPUT_CURVE_SCAN_LIMIT};
use crate::transfer::test_support::{grid_from, load_reference_in};

#[test]
fn output_curve_detection_matches_python() {
    let golden = load_reference_in("parse", "output_curve");
    let cases = golden["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "golden has no cases");

    for (i, case) in cases.iter().enumerate() {
        let grid = grid_from(&case["grid"]);
        let name = case["name"].as_str().unwrap();
        let expected = case["expected"].as_bool().unwrap();
        assert_eq!(
            looks_like_output_curve(&grid, name),
            expected,
            "case {i} ({name})"
        );
    }
}

#[test]
fn output_curve_scan_is_bounded() {
    // Structural guard mirroring test_looks_like_output_curve_stops_scanning_at_limit:
    // a marker placed past the limit must not be detected.
    const {
        assert!(
            OUTPUT_CURVE_SCAN_LIMIT < 50,
            "scan limit must stay small (Phase 7 perf fix)"
        );
    }
    let mut grid: Grid = (0..OUTPUT_CURVE_SCAN_LIMIT + 5)
        .map(|i| vec!["junk".to_string(), i.to_string()])
        .collect();
    grid.push(vec!["Vd".to_string(), "abs_Id".to_string()]); // marker beyond the limit
    assert!(!looks_like_output_curve(&grid, "transfer.xlsx"));
}

#[test]
fn transfer_shaped_header_clears_an_output_looking_name() {
    // A stem like "run2o" matches the digit+'o' output naming convention, but the
    // grid says transfer (Vg + Id, no Vd) — an Id-Vd export always carries a Vd
    // column, so content wins and the file stays loadable as a transfer sweep.
    let grid = vec![
        vec!["Vg".to_string(), "Id".to_string()],
        vec!["0".to_string(), "1e-9".to_string()],
    ];
    assert!(!looks_like_output_curve(&grid, "run2o.csv"));
    assert!(!looks_like_output_curve(&grid, "sweep_output.csv"));
}

#[test]
fn setup_name_id_vd_rejects_output_even_when_vg_is_present() {
    let grid = vec![
        vec!["Setup Name:".to_string(), "Id-Vd-low-5".to_string()],
        vec!["Date:".to_string(), "45637.524201388886".to_string()],
        vec![
            "vd".to_string(),
            "vg".to_string(),
            "id".to_string(),
            "abs_id".to_string(),
        ],
        vec![
            "0".to_string(),
            "-5".to_string(),
            "1.20364E-8".to_string(),
            "1.20364E-8".to_string(),
        ],
    ];

    assert!(looks_like_output_curve(&grid, "2-6o.xlsx"));
}
