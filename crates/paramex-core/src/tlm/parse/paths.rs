//! TLM workbook path discovery and group/length derivation policy.

use std::path::{Path, PathBuf};

use crate::tlm::types::TlmParseError;

/// Recursively collect `*.xlsx` under `root`, sorted by relative POSIX path,
/// lowercased (`parser.py:discover_workbooks`).
pub(in crate::tlm) fn discover_workbooks(root: &Path) -> Result<Vec<PathBuf>, TlmParseError> {
    if !root.exists() {
        return Err(TlmParseError(format!(
            "TLM data folder does not exist: {}",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(TlmParseError(format!(
            "TLM data path is not a folder: {}",
            root.display()
        )));
    }
    let mut out: Vec<PathBuf> = Vec::new();
    collect_xlsx(root, &mut out);
    out.sort_by_key(|p| rel_posix(p, root).to_lowercase());
    Ok(out)
}

fn collect_xlsx(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_xlsx(&path, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("xlsx"))
            == Some(true)
        {
            out.push(path);
        }
    }
}

/// Relative path with `/` separators (the sort key; matches `as_posix()`).
fn rel_posix(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    path_parts(rel).join("/")
}

/// Relative path with the OS separator (backslash on Windows): error-message
/// prefixes here, and the status.csv `file` column via `tlm::service`.
pub(in crate::tlm) fn rel_os(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Parsed `(group, length_um)` for a workbook path under a TLM root.
pub(super) fn workbook_group_length(
    path: &Path,
    root: &Path,
) -> Result<(String, f64), TlmParseError> {
    let rel = path.strip_prefix(root).map_err(|_| {
        TlmParseError(format!(
            "{} is not under TLM root {}",
            path.display(),
            root.display()
        ))
    })?;
    let parts = path_parts(rel);
    if parts.len() < 2 {
        return Err(TlmParseError(format!(
            "{} is not under length/file folders",
            rel_os(path, root)
        )));
    }
    let (group, length_name) = group_length_from_parts(&parts, root);
    // A folder literally named "nan"/"inf" parses to a non-finite f64; reject it (same as
    // unparseable) as non-numeric so the file surfaces a cause-accurate error instead of
    // silently dropping from / poisoning the fit (inf -> NaN slope). Mirrors the
    // `.ok().filter(|l| l.is_finite())` idiom in `path_group_length`.
    let length_um = length_name
        .parse::<f64>()
        .ok()
        .filter(|l| l.is_finite())
        .ok_or_else(|| {
            TlmParseError(format!(
                "{} has non-numeric channel length folder",
                rel_os(path, root)
            ))
        })?;
    Ok((group, length_um))
}

fn group_length_from_parts(parts: &[&str], root: &Path) -> (String, String) {
    if parts.len() >= 3 {
        (parts[0].to_string(), parts[1].to_string())
    } else if !parts.is_empty() {
        (root_name(root).to_string(), parts[0].to_string())
    } else {
        (root_name(root).to_string(), String::new())
    }
}

/// `(group, length|None)` for status rows on parse failure (`parser.py:_path_group_length`).
pub(in crate::tlm) fn path_group_length(path: &Path, root: &Path) -> (String, Option<f64>) {
    let Ok(rel) = path.strip_prefix(root) else {
        return (String::new(), None);
    };
    let parts = path_parts(rel);
    if parts.len() < 2 {
        return (parts.first().copied().unwrap_or("").to_string(), None);
    }
    let (group, length_name) = group_length_from_parts(&parts, root);
    (
        group,
        length_name.parse::<f64>().ok().filter(|l| l.is_finite()),
    )
}

fn path_parts(path: &Path) -> Vec<&str> {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect()
}

fn root_name(root: &Path) -> &str {
    root.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_length_folder_is_rejected() {
        let root = Path::new("root");
        // A "nan"/"inf" length folder must not parse to Some(non-finite).
        assert_eq!(
            path_group_length(Path::new("root/proc/nan/d.xlsx"), root).1,
            None
        );
        assert_eq!(
            path_group_length(Path::new("root/proc/inf/d.xlsx"), root).1,
            None
        );
        // A numeric folder still parses.
        assert_eq!(
            path_group_length(Path::new("root/proc/120/d.xlsx"), root),
            ("proc".to_string(), Some(120.0))
        );
    }
}
