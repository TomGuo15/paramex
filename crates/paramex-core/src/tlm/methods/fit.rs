//! TLM line-fit parity math.

/// Closed-form OLS line fit -> `(slope, intercept)`, equivalent to
/// `np.polyfit(x, y, 1)` within `rtol` (spec parity rule). Requires `x.len() >= 2`.
///
/// Deliberately parallel to `numerics::linear_fit_with_r2` (the transfer
/// engine's fit): same normal-equation arithmetic, but the R2 NaN gates differ
/// (`total <= 0.0` here vs `ss_tot <= FLOAT_EPSILON` there) and both engines are
/// golden-frozen - don't consolidate without re-proving both corpora byte-stable.
pub(in crate::tlm) fn polyfit1(x: &[f64], y: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let sx: f64 = x.iter().sum();
    let sy: f64 = y.iter().sum();
    let sxx: f64 = x.iter().map(|v| v * v).sum();
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| a * b).sum();
    let denom = n * sxx - sx * sx;
    let slope = (n * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / n;
    (slope, intercept)
}

/// `1 - SS_res/SS_tot` (`methods.py:_r_squared`). NaN when total variation is 0.
pub(in crate::tlm) fn r_squared(y: &[f64], predicted: &[f64]) -> f64 {
    let residual: f64 = y
        .iter()
        .zip(predicted)
        .map(|(a, b)| (a - b) * (a - b))
        .sum();
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let total: f64 = y.iter().map(|v| (v - mean) * (v - mean)).sum();
    if total <= 0.0 {
        return f64::NAN;
    }
    1.0 - residual / total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyfit1_matches_known_line() {
        let (slope, intercept) = polyfit1(&[1.0, 2.0, 4.0], &[8.0, 11.0, 17.0]);
        assert!((slope - 3.0).abs() < 1e-9);
        assert!((intercept - 5.0).abs() < 1e-9);
    }

    #[test]
    fn r_squared_is_one_for_a_perfect_fit_and_nan_for_constant_data() {
        assert!((r_squared(&[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-12);
        assert!(r_squared(&[2.0, 2.0, 2.0], &[2.0, 2.0, 2.0]).is_nan());
    }
}
