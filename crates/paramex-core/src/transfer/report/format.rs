//! Number formatters (`core/formatting.py` + `gui/formatting.py`).
//!
//! All take `Option<f64>` where `None` mirrors Python's `None`/non-numeric
//! input. NaN, ±Inf, and `None` collapse to each formatter's sentinel
//! (`"NA"`, or `"0"` for counts), exactly as the Python helpers do.

/// `fmt` with default precision 2 (`core/formatting.py` `fmt`).
pub(super) fn fmt(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("{v:.2}"),
        _ => "NA".to_string(),
    }
}

/// Engineering-notation current (mA→fA), or `"NA"` (`gui/formatting.py:11-30`).
/// Below 1 fA, falls back to Python-style scientific notation + `" A"`. The unit
/// `µ` is U+00B5 (MICRO SIGN), matching the Python source.
pub(super) fn fmt_engineering_current(value: Option<f64>) -> String {
    let number = match value {
        Some(v) if v.is_finite() => v,
        _ => return "NA".to_string(),
    };
    let abs_value = number.abs();
    const UNITS: [(f64, &str); 5] = [
        (1e-3, "mA"),
        (1e-6, "\u{00B5}A"),
        (1e-9, "nA"),
        (1e-12, "pA"),
        (1e-15, "fA"),
    ];
    for (scale, unit) in UNITS {
        if abs_value >= scale {
            return format!("{:.2} {}", number / scale, unit);
        }
    }
    format!("{} A", format_sci_2e(number))
}

/// Plain-text power-of-ten for CSV (`gui/formatting.py:38-40`): `× 10^exp`.
pub(super) fn fmt_power_of_ten_text(value: Option<f64>) -> String {
    fmt_power_of_ten_impl(value)
}

/// Shared mantissa+exponent power-of-ten (`gui/formatting.py:43-53`). `"NA"` for
/// `None`/non-finite/`x ≤ 0`. Exponent = `floor(log10(x))` (floating-point edge case:
/// near exact powers the mantissa may render `10.00 × 10ⁿ`). `×` is U+00D7.
fn fmt_power_of_ten_impl(value: Option<f64>) -> String {
    let number = match value {
        Some(v) if v.is_finite() && v > 0.0 => v,
        _ => return "NA".to_string(),
    };
    let exponent = number.log10().floor() as i64;
    let mantissa = number / 10f64.powi(exponent as i32);
    format!("{:.2} \u{00D7} 10^{}", mantissa, exponent)
}

/// Integer count string, `"0"` for `None`/non-finite (`gui/formatting.py:56-62`).
/// `int()` truncates toward zero (matches `value as i64`).
pub(super) fn format_count(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => (v as i64).to_string(),
        _ => "0".to_string(),
    }
}

/// Reformat Rust `{:.2e}` to Python `f"{n:.2e}"`: lowercase `e`, explicit sign,
/// exponent zero-padded to ≥2 digits. The mantissa is identical between Rust and
/// Python (verified). Examples: `0.0 → "0.00e+00"`, `1e-15 → "1.00e-15"`,
/// `-0.125 → "-1.25e-01"`. Non-finite input returns Python's strings
/// (`"nan"` / `"inf"` / `"-inf"`).
pub(super) fn format_sci_2e(n: f64) -> String {
    if !n.is_finite() {
        // Rust formats non-finite floats with no exponent, so the 'e' split
        // below would panic; Python f"{n:.2e}" yields these strings instead.
        return if n.is_nan() {
            "nan".to_string()
        } else if n > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    let raw = format!("{:.2e}", n); // e.g. "1.25e-1", "0.00e0", "-1.25e-1"
    let (mantissa, exp) = raw.split_once('e').expect("rust {:e} always has 'e'");
    let exp_i: i32 = exp.parse().expect("exponent is an integer");
    let sign = if exp_i < 0 { '-' } else { '+' };
    format!("{}e{}{:02}", mantissa, sign, exp_i.abs())
}
