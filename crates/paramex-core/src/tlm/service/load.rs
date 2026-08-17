//! TLM dataset loading orchestration.

use std::path::Path;

use crate::tlm::format::fmt_g;
use crate::tlm::parse::{discover_workbooks, parse_workbook, path_group_length, rel_os};
use crate::tlm::types::{FileStatus, Status, TlmCurve, TlmDataset, TlmParseError, VdSource};

/// Load all TLM workbooks under `root` in deterministic (discovery) order
/// (`service.py:load_dataset` + `_load_dataset_sequential`).
pub fn load_dataset(root: &Path, fallback_vd: Option<f64>) -> Result<TlmDataset, TlmParseError> {
    let workbooks = discover_workbooks(root)?;
    let mut curves: Vec<TlmCurve> = Vec::new();
    let mut statuses: Vec<FileStatus> = Vec::new();
    for wb in &workbooks {
        match parse_workbook(wb, root, fallback_vd) {
            Ok(curve) => {
                let message = if curve.vd_source() == VdSource::Fallback {
                    format!("Loaded with fallback V_D={} V", fmt_g(curve.vd()))
                } else {
                    "Loaded".to_string()
                };
                statuses.push(FileStatus {
                    file: rel_os(wb, root),
                    group: curve.group().to_string(),
                    length_um: Some(curve.length_um()),
                    status: Status::Ok,
                    message,
                    vd_source: curve.vd_source(),
                });
                curves.push(curve);
            }
            Err(exc) => {
                let (group, length_um) = path_group_length(wb, root);
                statuses.push(FileStatus {
                    file: rel_os(wb, root),
                    group,
                    length_um,
                    status: Status::Error,
                    message: exc.0,
                    vd_source: VdSource::Unread,
                });
            }
        }
    }
    TlmDataset::try_new(root.display().to_string(), curves, statuses)
}
