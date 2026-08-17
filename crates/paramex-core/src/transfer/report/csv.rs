//! Byte-exact sectioned CSV export (`gui/exporter.py`).

use super::table::results_to_report_sections;
use crate::transfer::types::MetricResult;

/// Serialise the sectioned report to CSV bytes (`exporter.py:54-79`).
///
/// Byte layout: a leading UTF-8 BOM (`EF BB BF`); CRLF (`\r\n`)
/// row terminators always; `QUOTE_MINIMAL` quoting; per section a title row, a
/// header row, the data rows, and a single empty row **between** sections only
/// (no trailing blank). Empty results → empty bytes (Python opens with the
/// `utf-8-sig` codec, which emits the BOM lazily on the **first** write; with no
/// sections nothing is written, so the file is empty — the golden confirms `b""`,
/// not a lone BOM).
pub(in crate::transfer) fn export_results_bytes(results: &[MetricResult]) -> Vec<u8> {
    let sections = results_to_report_sections(results);
    let mut out: Vec<u8> = Vec::new();
    let n = sections.len();
    // BOM is written only when there is at least one row to write, matching the
    // `utf-8-sig` lazy-BOM behaviour of the Python writer (empty -> no BOM).
    if n > 0 {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    for (i, section) in sections.iter().enumerate() {
        write_row(&mut out, std::slice::from_ref(&section.title));
        write_row(&mut out, &section.header);
        for row in &section.rows {
            write_row(&mut out, row);
        }
        if i + 1 < n {
            write_row(&mut out, &[]); // single empty row between sections
        }
    }
    out
}

/// Write one CSV record: comma-joined `QUOTE_MINIMAL` fields + CRLF. An empty
/// `fields` slice writes just the terminator (matches `csv.writer.writerow([])`).
pub(super) fn write_row(out: &mut Vec<u8>, fields: &[String]) {
    let joined = fields
        .iter()
        .map(|f| quote_minimal(f))
        .collect::<Vec<_>>()
        .join(",");
    out.extend_from_slice(joined.as_bytes());
    out.extend_from_slice(b"\r\n");
}

/// `QUOTE_MINIMAL`: quote iff the field contains the delimiter, a quote, or a
/// CR/LF; internal quotes are doubled (`csv` module default behaviour).
fn quote_minimal(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\r') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}
