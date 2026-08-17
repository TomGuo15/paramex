//! Guards: the committed TLM data is synthetic (no real labels/IDs) and still
//! exercises the warning / error-status / multi-device code paths.

use std::fs;
use std::path::{Path, PathBuf};

use crate::common::tlm_reference_dir;

/// Substrings/patterns that must never appear in committed TLM data again.
fn contains_real_token(s: &str) -> bool {
    if s.contains("blade") || s.contains("spin_coating") {
        return true;
    }
    // Real device IDs looked like "7_2_4" / "3_1_3": digit_digit_digit.
    let bytes = s.as_bytes();
    bytes.windows(5).any(|w| {
        w[0].is_ascii_digit()
            && w[1] == b'_'
            && w[2].is_ascii_digit()
            && w[3] == b'_'
            && w[4].is_ascii_digit()
    })
}

fn read(path: PathBuf) -> String {
    String::from_utf8_lossy(&fs::read(&path).expect("file present")).into_owned()
}

/// Minimal recursive file walk (avoids adding a walkdir dependency).
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn corpus_and_oracle_contain_no_real_labels() {
    let root = tlm_reference_dir();
    for entry in walk(&root.join("corpus")) {
        let name = entry.to_string_lossy().to_string();
        assert!(!contains_real_token(&name), "real token in path: {name}");
    }
    for f in ["result.csv", "sweep.csv", "length_points.csv", "status.csv"] {
        let text = read(root.join("oracle").join(f));
        assert!(!contains_real_token(&text), "real token in oracle/{f}");
    }
}

#[test]
fn oracle_shape_and_behavior_preserved() {
    let root = tlm_reference_dir();
    let result = read(root.join("oracle").join("result.csv"));
    let status = read(root.join("oracle").join("status.csv"));
    let points = read(root.join("oracle").join("length_points.csv"));

    // 4 process groups, all present.
    for g in ["process_a", "process_b", "process_c", "process_d"] {
        assert!(result.contains(g), "missing group {g} in result.csv");
    }
    // Warning split: at least one poor-fit warning AND at least one clean group.
    assert!(result.contains("Poor TLM fit"), "no poor-fit warning row");
    // A clean group row ends with the trailing empty warnings column.
    assert!(
        result
            .lines()
            .any(|l| l.starts_with("process_b") && l.trim_end().ends_with(',')),
        "no clean (no-warning) group row"
    );
    // Error-status path preserved (exactly one error row, vd_source unread).
    let errors = status.lines().filter(|l| l.contains(",error,")).count();
    assert_eq!(errors, 1, "expected exactly one error status row");
    assert!(
        status.contains("unread"),
        "error row should be vd_source=unread"
    );
    // Multi-device aggregation preserved: device_count column is 2.
    assert!(
        points
            .lines()
            .skip(1)
            .all(|l| l.split(',').any(|c| c == "2")),
        "expected device_count=2 on every length point"
    );
}
