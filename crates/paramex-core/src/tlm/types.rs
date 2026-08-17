//! TLM domain types. Owned `Vec<f64>` replace numpy arrays.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::tlm::methods::available_vg_values;

/// Raised when a TLM workbook cannot be interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlmParseError(pub String);

impl fmt::Display for TlmParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for TlmParseError {}

/// Per-workbook ingest outcome (`domain.py:Status`). String form matches the CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Error,
}
impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Error => "error",
        }
    }
}

/// Where a curve's drain bias came from (`domain.py:VdSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdSource {
    Setup,
    Fallback,
    Unread,
}
impl VdSource {
    pub fn as_str(self) -> &'static str {
        match self {
            VdSource::Setup => "setup",
            VdSource::Fallback => "fallback",
            VdSource::Unread => "unread",
        }
    }
}

/// One finite measured row from a TLM workbook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TlmSample {
    vg: f64,
    abs_id: f64,
    abs_is: f64,
}

impl TlmSample {
    pub fn try_new(vg: f64, abs_id: f64, abs_is: f64) -> Result<Self, TlmParseError> {
        if !vg.is_finite() {
            return Err(TlmParseError(
                "TLM sample V_G must be a finite number".to_string(),
            ));
        }
        if !abs_id.is_finite() {
            return Err(TlmParseError(
                "TLM sample abs_id must be a finite number".to_string(),
            ));
        }
        if !abs_is.is_finite() {
            return Err(TlmParseError(
                "TLM sample abs_is must be a finite number".to_string(),
            ));
        }
        Ok(Self { vg, abs_id, abs_is })
    }

    pub fn vg(self) -> f64 {
        self.vg
    }

    pub fn abs_id(self) -> f64 {
        self.abs_id
    }

    pub fn abs_is(self) -> f64 {
        self.abs_is
    }
}

/// One parsed workbook with non-empty samples sorted by ascending `V_G`.
#[derive(Debug, Clone, PartialEq)]
pub struct TlmCurve {
    file_path: String,
    group: String,
    length_um: f64,
    device_id: String,
    samples: Vec<TlmSample>,
    vd: f64,
    vd_source: VdSource,
}

impl TlmCurve {
    pub fn try_new(
        file_path: String,
        group: String,
        length_um: f64,
        mut samples: Vec<TlmSample>,
        vd: f64,
        vd_source: VdSource,
    ) -> Result<Self, TlmParseError> {
        if file_path.trim().is_empty() {
            return Err(TlmParseError(
                "TLM curve file path must not be empty".to_string(),
            ));
        }
        if group.trim().is_empty() {
            return Err(TlmParseError(
                "TLM curve group must not be empty".to_string(),
            ));
        }
        if !length_um.is_finite() {
            return Err(TlmParseError(
                "TLM curve channel length must be a finite number".to_string(),
            ));
        }
        if samples.is_empty() {
            return Err(TlmParseError(
                "TLM curve must contain at least one sample".to_string(),
            ));
        }
        let vd = valid_vd(vd, "TLM curve V_D")?;
        if vd_source == VdSource::Unread {
            return Err(TlmParseError(
                "TLM curve V_D source must be setup or fallback".to_string(),
            ));
        }
        let device_id = Path::new(&file_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.trim().is_empty())
            .ok_or_else(|| TlmParseError("TLM curve file path must identify a device".to_string()))?
            .to_string();
        samples.sort_by(|left, right| {
            left.vg
                .partial_cmp(&right.vg)
                .expect("validated TLM sample voltages are finite")
        });

        Ok(Self {
            file_path,
            group,
            length_um,
            device_id,
            samples,
            vd,
            vd_source,
        })
    }

    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn length_um(&self) -> f64 {
        self.length_um
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn samples(&self) -> &[TlmSample] {
        &self.samples
    }

    pub fn vd(&self) -> f64 {
        self.vd
    }

    pub fn vd_source(&self) -> VdSource {
        self.vd_source
    }

    /// MATLAB-compatible channel current at the gate voltage nearest `selected_vg`
    /// (`domain.py:TlmCurve.current_at`). Returns `(current, actual_vg)`; current is
    /// `min(|abs_id|, |abs_is|)`.
    pub fn current_at(&self, selected_vg: f64) -> (f64, f64) {
        let size = self.samples.len();
        let pos = self
            .samples
            .partition_point(|sample| sample.vg < selected_vg);
        let index = if pos == 0 {
            0
        } else if pos == size {
            size - 1
        } else {
            let left = self.samples[pos - 1].vg;
            let right = self.samples[pos].vg;
            if (right - selected_vg).abs() < (selected_vg - left).abs() {
                pos
            } else {
                pos - 1
            }
        };
        let sample = self.samples[index];
        let current = sample.abs_id.abs().min(sample.abs_is.abs());
        (current, sample.vg)
    }
}

/// File-status row (`domain.py:FileStatus`).
#[derive(Debug, Clone, PartialEq)]
pub struct FileStatus {
    pub file: String, // relative path string, OS separators (Windows backslash)
    pub group: String,
    pub length_um: Option<f64>,
    pub status: Status,
    pub message: String,
    pub vd_source: VdSource,
}

/// Bundle of parsed workbooks (`domain.py:TlmDataset`).
#[derive(Debug, Clone, PartialEq)]
pub struct TlmDataset {
    root: String,
    curves: Vec<TlmCurve>,
    statuses: Vec<FileStatus>,
    vg_values: Vec<f64>,
}

/// Result of removing one workbook from an admitted TLM dataset.
///
/// `dataset` is `None` when the removal consumed the final successfully parsed
/// curve, so callers cannot retain an error-only aggregate.
#[must_use = "workbook removal must retain the returned dataset or handle its terminal state"]
#[derive(Debug, Clone, PartialEq)]
pub struct TlmDatasetRemoval {
    pub dataset: Option<TlmDataset>,
    pub removed_statuses: usize,
}

impl TlmDataset {
    /// Admit a coherent workbook aggregate and derive its measured gate-voltage set.
    pub fn try_new(
        root: String,
        curves: Vec<TlmCurve>,
        statuses: Vec<FileStatus>,
    ) -> Result<Self, TlmParseError> {
        if root.trim().is_empty() {
            return Err(TlmParseError(
                "TLM dataset root must not be empty".to_string(),
            ));
        }
        if curves.is_empty() {
            return Err(TlmParseError(
                "No valid TLM workbooks were found.".to_string(),
            ));
        }

        let root_path = Path::new(&root);
        let mut curves_by_file = BTreeMap::<PathBuf, &TlmCurve>::new();
        for curve in &curves {
            let relative = Path::new(curve.file_path())
                .strip_prefix(root_path)
                .map_err(|_| {
                    TlmParseError(format!(
                        "TLM curve {} is not under dataset root {root}",
                        curve.file_path()
                    ))
                })?;
            if !is_clean_relative_path(relative) {
                return Err(TlmParseError(format!(
                    "TLM curve {} has an invalid workbook identity",
                    curve.file_path()
                )));
            }
            if curves_by_file
                .insert(relative.to_path_buf(), curve)
                .is_some()
            {
                return Err(TlmParseError(format!(
                    "TLM dataset contains duplicate curve identity {}",
                    relative.display()
                )));
            }
        }

        let mut statuses_by_file = BTreeMap::<PathBuf, &FileStatus>::new();
        for status in &statuses {
            let relative = Path::new(&status.file);
            if !is_clean_relative_path(relative) {
                return Err(TlmParseError(format!(
                    "TLM status has an invalid workbook identity {}",
                    status.file
                )));
            }
            if statuses_by_file
                .insert(relative.to_path_buf(), status)
                .is_some()
            {
                return Err(TlmParseError(format!(
                    "TLM dataset contains duplicate status identity {}",
                    status.file
                )));
            }

            match status.status {
                Status::Ok => {
                    let curve = curves_by_file.get(relative).ok_or_else(|| {
                        TlmParseError(format!(
                            "Successful TLM status {} has no matching curve",
                            status.file
                        ))
                    })?;
                    if status.group != curve.group()
                        || status.length_um != Some(curve.length_um())
                        || status.vd_source != curve.vd_source()
                    {
                        return Err(TlmParseError(format!(
                            "TLM status metadata does not match curve {}",
                            status.file
                        )));
                    }
                }
                Status::Error => {
                    if curves_by_file.contains_key(relative) {
                        return Err(TlmParseError(format!(
                            "Failed TLM status {} must not have a curve",
                            status.file
                        )));
                    }
                }
            }
        }

        for relative in curves_by_file.keys() {
            if !statuses_by_file
                .get(relative)
                .is_some_and(|status| status.status == Status::Ok)
            {
                return Err(TlmParseError(format!(
                    "TLM curve {} has no matching successful status",
                    relative.display()
                )));
            }
        }

        let vg_values = available_vg_values(&curves);
        Ok(Self {
            root,
            curves,
            statuses,
            vg_values,
        })
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn curves(&self) -> &[TlmCurve] {
        &self.curves
    }

    pub fn statuses(&self) -> &[FileStatus] {
        &self.statuses
    }

    pub fn vg_values(&self) -> &[f64] {
        &self.vg_values
    }

    /// Consume this aggregate to remove one exact relative workbook identity.
    /// The final successful-curve removal is terminal: residual failed-workbook
    /// statuses are included in the count and no empty aggregate is returned.
    pub fn remove_workbook(mut self, relative_file: &str) -> TlmDatasetRemoval {
        let removed = self
            .statuses
            .iter()
            .filter(|status| status.file == relative_file)
            .count();
        if removed == 0 {
            return TlmDatasetRemoval {
                dataset: Some(self),
                removed_statuses: 0,
            };
        }

        self.statuses.retain(|status| status.file != relative_file);

        let relative_file = Path::new(relative_file);
        self.curves.retain(|curve| {
            !Path::new(curve.file_path())
                .strip_prefix(Path::new(&self.root))
                .is_ok_and(|relative| relative == relative_file)
        });
        if self.curves.is_empty() {
            return TlmDatasetRemoval {
                dataset: None,
                removed_statuses: removed + self.statuses.len(),
            };
        }
        self.vg_values = available_vg_values(&self.curves);
        TlmDatasetRemoval {
            dataset: Some(self),
            removed_statuses: removed,
        }
    }
}

fn is_clean_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// One Length-Points row (`domain.py:LengthPoint`). Max-policy fields first, then median.
#[derive(Debug, Clone, PartialEq)]
pub struct LengthPoint {
    pub group: String,
    pub length_um: f64,
    pub selected_vg: f64,
    pub actual_vg: f64,
    pub current_a: f64,
    pub rtotal_ohm: f64,
    pub current_median_a: f64,
    pub rtotal_median_ohm: f64,
    pub device_count: usize,
    pub selected_file: String,
}

/// One group's TLM fit at one V_G (`domain.py:GroupAnalysis`).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupAnalysis {
    pub group: String,
    pub selected_vg: f64,
    pub points: Vec<LengthPoint>,
    pub intercept_ohm: f64,
    pub rc_per_contact_ohm: f64,
    pub slope_ohm_per_um: f64,
    pub r_squared: f64,
    pub intercept_median_ohm: f64,
    pub rc_per_contact_median_ohm: f64,
    pub slope_median_ohm_per_um: f64,
    pub r_squared_median: f64,
    pub warnings: Vec<String>,
}

/// Single-V_G analysis result (`domain.py:TlmAnalysisResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct TlmAnalysisResult {
    pub root: String,
    pub selected_vg: f64,
    pub vg_values: Vec<f64>,
    pub groups: Vec<GroupAnalysis>,
    pub statuses: Vec<FileStatus>,
}
impl TlmAnalysisResult {
    pub fn first_group_name(&self) -> Option<&str> {
        self.groups.first().map(|g| g.group.as_str())
    }

    pub fn has_group(&self, name: &str) -> bool {
        self.groups.iter().any(|g| g.group == name)
    }

    pub fn group(&self, name: &str) -> Option<&GroupAnalysis> {
        self.groups.iter().find(|g| g.group == name)
    }
}

/// One sweep row (`domain.py:VoltageSweepPoint`) — same fit shape as GroupAnalysis.
#[derive(Debug, Clone, PartialEq)]
pub struct VoltageSweepPoint {
    pub group: String,
    pub selected_vg: f64,
    pub intercept_ohm: f64,
    pub rc_per_contact_ohm: f64,
    pub slope_ohm_per_um: f64,
    pub r_squared: f64,
    pub intercept_median_ohm: f64,
    pub rc_per_contact_median_ohm: f64,
    pub slope_median_ohm_per_um: f64,
    pub r_squared_median: f64,
    pub valid_lengths: usize,
    pub warnings: Vec<String>,
}

/// Full-sweep result (`domain.py:TlmSweepResult`).
#[derive(Debug, Clone, PartialEq)]
pub struct TlmSweepResult {
    pub root: String,
    pub vg_values: Vec<f64>,
    pub points: Vec<VoltageSweepPoint>,
}

/// Coerce a candidate drain bias to a finite, nonzero f64 (`domain.py:_valid_vd`).
pub fn valid_vd(value: f64, label: &str) -> Result<f64, TlmParseError> {
    if !value.is_finite() {
        return Err(TlmParseError(format!("{label} must be a finite number")));
    }
    if value == 0.0 {
        return Err(TlmParseError(format!("{label} must be nonzero")));
    }
    Ok(value)
}
