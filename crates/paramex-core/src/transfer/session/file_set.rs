//! Loaded-file identity: source paths, curve fingerprints, and deduplication.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::shared::same_source_path;
use crate::transfer::types::ParsedCurve;

/// Case-folded name. `to_lowercase` matches casefold for ASCII/Latin; full
/// Unicode casefold is not supported.
fn fold_name(name: &str) -> String {
    name.to_lowercase()
}

/// `(folded name, sha256-hex)` fingerprint (`file_set.py:63-68`). SHA-256 over
/// little-endian f64 bytes — `vg` in full, then `id_abs` — then the folded name.
pub(super) fn curve_fingerprint(curve: &ParsedCurve) -> (String, String) {
    let mut hasher = Sha256::new();
    for &v in &curve.vg {
        hasher.update(v.to_le_bytes());
    }
    for &v in &curve.id_abs {
        hasher.update(v.to_le_bytes());
    }
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    (fold_name(&curve.name), hex)
}

/// Curves that share a resolved source path OR a numerical fingerprint
/// (`file_set.py:52-60`). The fingerprint comparison is tuple-equality (folded
/// name AND hash must match), matching `_curve_fingerprint(l) == _curve_fingerprint(r)`.
#[cfg(test)]
pub(super) fn curves_match(left: &ParsedCurve, right: &ParsedCurve) -> bool {
    curves_match_fp(left, right, &curve_fingerprint(right))
}

/// [`curves_match`] with `right`'s fingerprint already computed, so a batch check
/// of one incoming curve against many loaded curves hashes it only once.
fn curves_match_fp(left: &ParsedCurve, right: &ParsedCurve, right_fp: &(String, String)) -> bool {
    if let (Some(l), Some(r)) = (&left.source_path, &right.source_path) {
        if same_source_path(l, r) {
            return true;
        }
    }
    curve_fingerprint(left) == *right_fp
}

/// True when `curve` matches any already-loaded curve (`file_set.py:47-49`).
pub(super) fn curve_loaded<'a>(
    items: impl IntoIterator<Item = &'a ParsedCurve>,
    curve: &ParsedCurve,
) -> bool {
    let curve_fp = curve_fingerprint(curve);
    items
        .into_iter()
        .any(|c| curves_match_fp(c, curve, &curve_fp))
}

/// True when any loaded curve was sourced from `path` (`file_set.py:37-44`).
pub(super) fn source_path_loaded<'a>(
    items: impl IntoIterator<Item = &'a ParsedCurve>,
    path: &Path,
) -> bool {
    items.into_iter().any(|curve| {
        curve
            .source_path
            .as_deref()
            .is_some_and(|source| same_source_path(source, path))
    })
}
