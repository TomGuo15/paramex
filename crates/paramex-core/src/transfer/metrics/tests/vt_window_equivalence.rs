use crate::transfer::metrics::vth::{select_elr_vt_window, DEFAULT_VT_R2_LADDER};
use crate::transfer::test_support::{f64_vec, load_reference_in, parse_f64};

#[test]
fn select_elr_vt_window_matches_reference_corpus() {
    let g = load_reference_in("metrics", "vt_window_equivalence");
    let cases = g["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 80, "expected 80 corpus seeds");

    let mut non_none = 0usize;
    for case in cases {
        let seed = case["seed"].as_u64().unwrap();
        let vg = f64_vec(&case["vg"]);
        let id_abs = f64_vec(&case["id_abs"]);
        // Production defaults: window_size=30, step=1, min_points=10, min_r2=0.99.
        let got = select_elr_vt_window(&vg, &id_abs, 30, 1, 10, 0.99, &DEFAULT_VT_R2_LADDER);
        let exp = &case["window"];
        if exp.is_null() {
            assert!(got.is_none(), "seed {seed}: expected None, got {got:?}");
        } else {
            let (lo, hi) = got.unwrap_or_else(|| panic!("seed {seed}: expected Some, got None"));
            let a = exp.as_array().unwrap();
            assert_eq!(lo, parse_f64(&a[0]), "seed {seed}: lo exact");
            assert_eq!(hi, parse_f64(&a[1]), "seed {seed}: hi exact");
            non_none += 1;
        }
    }
    // Non-degeneracy guard (lifted from test_corpus_is_non_degenerate): every
    // seed must select a real window, so the equivalence check is not vacuous.
    assert_eq!(
        non_none, 80,
        "expected all 80 seeds to select a window, got {non_none}"
    );
}
