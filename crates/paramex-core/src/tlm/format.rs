//! Shared TLM status and warning text formatting.

/// Approximates Python `f"{x:g}"` only for the value shapes fed to it today —
/// whole numbers and short decimals, in the warning + length labels in
/// `tlm::methods` and the "Loaded with fallback V_D=..." status message in
/// `tlm::service`. Integers print without a decimal point; anything else is
/// Rust shortest-roundtrip `Display`. NOT a full `%g`: no rounding to 6
/// significant digits and no scientific branch, so it diverges from Python for
/// long fractions (e.g. a measured V_G of -39.999999999 → Python "-40") and for
/// whole numbers with |x| >= 1e6 (Python "1e+06"). These strings are
/// user-visible warning text; the committed golden corpus does not currently
/// exercise them, so "completing" the %g port would not break today's goldens
/// but IS a behavior change — do it only with oracle coverage.
pub(super) fn fmt_g(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e16 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}
