use crate::shared::grid_ingest::split_single_column;
use crate::transfer::test_support::{grid_from, load_reference_in};

#[test]
fn split_single_column_matches_python() {
    let golden = load_reference_in("parse", "split_single_column");
    let cases = golden["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "golden has no cases");

    for (i, case) in cases.iter().enumerate() {
        let grid_in = grid_from(&case["grid_in"]);
        let expected = grid_from(&case["grid_out"]);
        assert_eq!(split_single_column(&grid_in), expected, "case {i}");
    }
}
