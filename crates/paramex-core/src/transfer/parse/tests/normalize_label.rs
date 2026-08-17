use crate::shared::grid_headers::normalize_label;
use crate::transfer::test_support::load_reference_in;

#[test]
fn normalize_label_matches_python() {
    let golden = load_reference_in("parse", "normalize_label");
    let cases = golden["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "golden has no cases");

    for case in cases {
        let input = case["input"].as_str().expect("input");
        let expected = case["expected"].as_str().expect("expected");
        assert_eq!(normalize_label(input), expected, "input {input:?}");
    }
}
