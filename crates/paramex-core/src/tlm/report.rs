//! TLM CSV row builders + writer.
//!
//! Uses the `csv` crate with QUOTE_MINIMAL-style output. Floats use Rust
//! formatting; NaN becomes an empty cell.

mod fit_tables;
mod length_points;
mod status;

pub use fit_tables::{result_csv, sweep_csv};
pub use length_points::length_points_csv;
pub use status::status_csv;

/// f64 cell: empty for NaN (pandas default `na_rep=""`), else Rust shortest repr.
fn fcell(x: f64) -> String {
    if x.is_nan() {
        String::new()
    } else {
        format!("{x}")
    }
}

fn write_csv(headers: &[&str], rows: Vec<Vec<String>>) -> Vec<u8> {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    w.write_record(headers).expect("header");
    for row in rows {
        w.write_record(&row).expect("row");
    }
    w.into_inner().expect("csv bytes")
}
