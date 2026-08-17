//! Transfer device geometry and extraction settings.

/// Per-file device geometry for mobility extraction (`models.py:25-36`
/// `DeviceGeometry`). Current app flows produce `"default"`, `"global"`, or
/// `"manual"`; report fixtures may still carry historical source strings.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceGeometry {
    pub width_um: f64,
    pub length_um: f64,
    pub source: String,
}

impl Default for DeviceGeometry {
    /// Mirrors the Python dataclass defaults (`width_um=1500.0`,
    /// `length_um=50.0`, `source="default"`).
    fn default() -> Self {
        DeviceGeometry {
            width_um: 1500.0,
            length_um: 50.0,
            source: "default".to_string(),
        }
    }
}

impl DeviceGeometry {
    /// W/L, or `NaN` when the channel length is non-positive (`models.py:33-36`).
    pub fn aspect_ratio(&self) -> f64 {
        if self.length_um > 0.0 {
            self.width_um / self.length_um
        } else {
            f64::NAN
        }
    }
}

/// Session-level geometry/capacitance settings (`models.py:39-55`
/// `ExtractionSettings`, frozen). Stored Cox is in nF/cm²; `cox_f_per_cm2`
/// converts to SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractionSettings {
    pub width_um: f64,
    pub length_um: f64,
    pub cox_nf_per_cm2: f64,
}

impl Default for ExtractionSettings {
    fn default() -> Self {
        ExtractionSettings {
            width_um: 1500.0,
            length_um: 50.0,
            cox_nf_per_cm2: 10.0,
        }
    }
}

impl ExtractionSettings {
    /// W/L, or `NaN` when the channel length is non-positive (`models.py:47-50`).
    pub fn aspect_ratio(&self) -> f64 {
        if self.length_um > 0.0 {
            self.width_um / self.length_um
        } else {
            f64::NAN
        }
    }

    /// Cox in farad/cm² (SI), converted from the stored nF/cm² (`models.py:52-55`).
    pub fn cox_f_per_cm2(&self) -> f64 {
        self.cox_nf_per_cm2 * 1e-9
    }
}
