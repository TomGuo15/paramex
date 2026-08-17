//! Cox capacitance math for Transfer extraction settings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoxError {
    NonPositiveOrNonFinite,
}

pub(in crate::transfer) fn validate_cox_nf_per_cm2(value: f64) -> Result<f64, CoxError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(CoxError::NonPositiveOrNonFinite)
    }
}

/// Series oxide capacitance in nF/cm² for one or more dielectric layers
/// (`models.py:198-211`). Returns `NaN` on no layers, any non-positive ε_r or
/// thickness, any non-finite input/intermediate, or a non-positive denominator.
/// Constants are pinned exactly:
/// `eps0 = 8.8541878128e-14 F/cm`, `thickness_cm = thickness_nm·1e-7`, final
/// `·1e9` (F/cm² → nF/cm²). Summation order matches Python (layer order).
pub fn calculate_stack_cox_nf_per_cm2(layers: &[(f64, f64)]) -> f64 {
    if layers.is_empty() {
        return f64::NAN;
    }
    let mut denominator = 0.0_f64;
    for &(epsilon_r, thickness_nm) in layers {
        if !epsilon_r.is_finite()
            || epsilon_r <= 0.0
            || !thickness_nm.is_finite()
            || thickness_nm <= 0.0
        {
            return f64::NAN;
        }
        let thickness_cm = thickness_nm * 1e-7;
        denominator += thickness_cm / epsilon_r;
    }
    if !denominator.is_finite() || denominator <= 0.0 {
        return f64::NAN;
    }
    let eps0_f_per_cm = 8.8541878128e-14;
    let cox = (eps0_f_per_cm / denominator) * 1e9;
    validate_cox_nf_per_cm2(cox).unwrap_or(f64::NAN)
}
