use crate::common::{assert_close, load_numpy_reference, parse_f64};
use paramex_core::shared::numpy_compat::gradient;

#[test]
fn gradient_matches_numpy_golden() {
    let g = load_numpy_reference("gradient");
    let cases = g["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "expected at least one golden case");

    for (ci, case) in cases.iter().enumerate() {
        let y: Vec<f64> = case["y"]
            .as_array()
            .expect("y array")
            .iter()
            .map(parse_f64)
            .collect();
        let x: Vec<f64> = case["x"]
            .as_array()
            .expect("x array")
            .iter()
            .map(parse_f64)
            .collect();
        let expected: Vec<f64> = case["expected"]
            .as_array()
            .expect("expected array")
            .iter()
            .map(parse_f64)
            .collect();

        let actual = gradient(&y, &x);

        assert_eq!(
            actual.len(),
            expected.len(),
            "case {ci}: output length mismatch (y.len()={})",
            y.len()
        );
        for (i, (&a, &e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_close(a, e, 1e-12, 1e-12);
            let _ = i;
        }
    }
}
