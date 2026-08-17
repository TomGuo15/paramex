use crate::transfer::report::schema::{column_keys, results_to_rows, Cell};
use crate::transfer::test_support::{load_reference_in, metric_result, parse_f64};
use serde_json::Value;

fn decode_cell(v: &Value) -> Cell {
    if v.is_null() {
        Cell::Null
    } else if v.is_string() {
        Cell::Text(v.as_str().unwrap().to_string())
    } else {
        Cell::Float(parse_f64(v))
    }
}

fn assert_cell_eq(got: &Cell, exp: &Cell, where_: &str) {
    match (got, exp) {
        (Cell::Float(a), Cell::Float(b)) => {
            assert!(a == b || (a.is_nan() && b.is_nan()), "{where_}: {a} != {b}");
        }
        _ => assert_eq!(got, exp, "{where_}"),
    }
}

#[test]
fn column_keys_match_python() {
    let g = load_reference_in("report", "schema");
    let expected: Vec<String> = g["column_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(column_keys(), expected);
}

#[test]
fn results_to_rows_match_python() {
    let g = load_reference_in("report", "schema");
    let results: Vec<_> = g["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(metric_result)
        .collect();
    let rows = results_to_rows(&results);
    let exp_rows = g["rows"].as_array().unwrap();
    assert_eq!(rows.len(), exp_rows.len(), "row count");
    for (ri, (row, exp_row)) in rows.iter().zip(exp_rows).enumerate() {
        let exp: Vec<Cell> = exp_row
            .as_array()
            .unwrap()
            .iter()
            .map(decode_cell)
            .collect();
        assert_eq!(row.len(), exp.len(), "row {ri} width");
        for (ci, (got, e)) in row.iter().zip(&exp).enumerate() {
            assert_cell_eq(got, e, &format!("row {ri} col {ci}"));
        }
    }
}

#[test]
fn unknown_column_key_is_graceful_not_a_panic() {
    use crate::transfer::report::schema::{key_index, value_for_column};

    assert_eq!(key_index("not_a_real_column"), None, "unknown key");
    assert!(key_index("filename").is_some(), "known key resolves");

    let g = load_reference_in("report", "schema");
    let result = metric_result(&g["results"].as_array().unwrap()[0]);
    assert!(
        matches!(
            value_for_column("not_a_real_column", &result, "Forward"),
            Cell::Null
        ),
        "unknown column key yields Cell::Null"
    );
}
