use crate::common::{assert_close, load_numpy_reference, parse_f64};
use paramex_core::shared::numpy_compat::nanmedian;

#[test]
fn nanmedian_matches_numpy_golden() {
    let golden = load_numpy_reference("nanmedian");
    let cases = golden["cases"].as_array().expect("cases array");
    for (i, case) in cases.iter().enumerate() {
        let vals: Vec<f64> = case["vals"]
            .as_array()
            .expect("vals array")
            .iter()
            .map(parse_f64)
            .collect();
        let expected = parse_f64(&case["expected"]);
        let actual = nanmedian(&vals);
        assert_close(actual, expected, 1e-12, 1e-12);
        // touch index so failures report which case
        let _ = i;
    }
}
