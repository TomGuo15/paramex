//! User-facing row labels, badges, and GUI messages.

/// The point-count label, exactly `"{N} pts"` (`file_list_panel.py:116`,
/// `f"{item.curve.vg.size} pts"`). No thousands separators. Used by the
/// Transfer file rows and the TLM group rows.
pub fn point_count_label(n: usize) -> String {
    format!("{n} pts")
}

/// The status-badge text: `"error"` when the file has an ingestion/extraction
/// error, else `"ok"` (`file_list_panel.py:112`).
pub fn status_badge(has_error: bool) -> &'static str {
    if has_error {
        "error"
    } else {
        "ok"
    }
}

/// The global-apply toast (`geometry_panel.py:195`, `f"Applied W/L to {n} file(s)."`).
/// `"file(s)"` is a fixed literal - no dynamic pluralization.
pub fn global_wl_message(count: usize) -> String {
    format!("Applied W/L to {count} file(s).")
}

pub fn exported_to(name: &str) -> String {
    format!("Exported to {name}")
}

pub fn loaded_files(count: usize) -> String {
    format!("Loaded {count} file(s)")
}

pub fn already_loaded(name: &str) -> String {
    format!("{name} is already loaded.")
}

pub fn removed_items(count: usize, noun: &str) -> String {
    format!("Removed {count} {noun}(s).")
}

pub fn cleared_error_rows() -> &'static str {
    "Cleared error row(s)."
}

pub fn cleared_model_fit_rows(devices: usize, pending: usize, errors: usize) -> String {
    format!(
        "Cleared {devices} device(s), {pending} pending measurement row(s), and {errors} error row(s)."
    )
}

pub fn model_fit_loaded_devices_summary(
    fitted: usize,
    already_loaded: usize,
    unfittable: usize,
    errors: usize,
    first_err: Option<&str>,
) -> String {
    with_first_error(
        format!(
            "Loaded {fitted} device(s); {already_loaded} already loaded, \
             {unfittable} unfittable, {errors} parse error(s)."
        ),
        fitted,
        errors,
        first_err,
    )
}

pub fn transfer_output_summary(
    attached: usize,
    unmatched: usize,
    ambiguous: usize,
    displaced: usize,
    errors: usize,
    first_err: Option<&str>,
) -> String {
    with_first_error(
        format!(
            "Transfer output: {attached} attached, {unmatched} unmatched, \
             {ambiguous} ambiguous, {displaced} displaced, {errors} error(s)."
        ),
        attached,
        errors,
        first_err,
    )
}

pub fn output_partial_fit_message(fitted: usize, total: usize) -> String {
    let unavailable = total.saturating_sub(fitted);
    let noun = if total == 1 { "line" } else { "lines" };
    format!("{unavailable} of {total} output {noun} unavailable")
}

pub fn model_fit_output_summary(
    attached: usize,
    unmatched: usize,
    ambiguous: usize,
    displaced: usize,
    unfittable: usize,
    errors: usize,
    first_err: Option<&str>,
) -> String {
    with_first_error(
        format!(
            "Output curves: {attached} attached, {unmatched} unmatched, {ambiguous} ambiguous, \
             {displaced} displaced, {unfittable} unfittable, {errors} error(s)."
        ),
        attached,
        errors,
        first_err,
    )
}

pub fn model_fit_output_refinement_failure(action: &str, recovered: usize) -> String {
    model_fit_refinement_failure(action, "output", recovered)
}

pub fn model_fit_dibl_refinement_failure(action: &str, recovered: usize) -> String {
    model_fit_refinement_failure(action, "DIBL", recovered)
}

fn model_fit_refinement_failure(action: &str, measurement: &str, recovered: usize) -> String {
    if recovered == 0 {
        return format!("{action} failed unexpectedly. Loaded device data was unchanged.");
    }
    format!(
        "{action} failed unexpectedly. Kept {recovered} parsed {measurement} measurement(s) \
         pending; loaded device data was unchanged."
    )
}

fn with_first_error(
    base: String,
    success_count: usize,
    errors: usize,
    first_err: Option<&str>,
) -> String {
    match first_err {
        Some(e) if success_count == 0 && errors > 0 => format!("{base} First error: {e}"),
        _ => base,
    }
}

pub const WL_POSITIVE_MESSAGE: &str = "W and L must be positive numbers.";
pub const WL_NUMERIC_MESSAGE: &str = "W and L must be numeric.";
pub const LOW_R2_MESSAGE: &str = "Low R\u{00B2}; review fit range.";
pub const LOW_R2_THRESHOLD: f64 = 0.95;
pub const ANALOG_GM_ERROR_LIMIT: f64 = 0.15;
pub const ANALOG_GDS_ERROR_LIMIT: f64 = 0.25;
pub const OUTPUT_NO_FINITE_POINTS_MESSAGE: &str = "No finite Id-Vd points";
pub const OUTPUT_SUMMARY_UNAVAILABLE_MESSAGE: &str = "Output fit unavailable";
pub const OUTPUT_FIT_FAILED_MESSAGE: &str = "Output fit failed.";
pub const ATTACHED_PENDING_OUTPUT_MESSAGE: &str = "Attached pending output.";
pub const REMOVED_PENDING_OUTPUT_MESSAGE: &str = "Removed pending output.";
pub const OUTPUT_MOVED_TO_PENDING_MESSAGE: &str = "Output moved to pending.";
pub const REMOVED_OUTPUT_MESSAGE: &str = "Removed output.";
pub const LEVEL62_EXTRACTION_FAILED_MESSAGE: &str = "Level 62 extraction failed.";
pub const MODEL_DEVICE_REQUIRED_MESSAGE: &str = "Select a fitted device first.";
pub const MODEL_PARAMETER_INVALID_MESSAGE: &str =
    "Parameter combination is outside the model's valid range.";
pub const MODEL_REFIT_FAILED_MESSAGE: &str = "Could not re-fit this device.";
pub const MODEL_CV_STALE_MESSAGE: &str =
    "Device changed or was removed while C-V extraction was running.";
pub const MODEL_SETUP_STALE_MESSAGE: &str =
    "Device changed or was removed while setup refinement was running.";
pub const WL_MODEL_INCOMPATIBLE_MESSAGE: &str =
    "W and L are incompatible with the current manual model parameters.";
pub const MODEL_DIBL_NO_MATCH_MESSAGE: &str = "No loaded device matches this DIBL file.";
pub const MODEL_DIBL_AMBIGUOUS_MESSAGE: &str = "Multiple loaded devices match this DIBL file.";
pub const MODEL_DIBL_STALE_MESSAGE: &str = "Device changed while DIBL refinement was running.";
pub const MODEL_DIBL_SOURCE_NAME_MESSAGE: &str =
    "The DIBL file has no visible file name and cannot be attached.";
pub const MODEL_DIBL_REAPPLY_FAILED_MESSAGE: &str =
    "The attached DIBL could not be reapplied. Detach or remove it, then retry.";
pub const MODEL_OUTPUT_DIBL_CONFLICT_MESSAGE: &str =
    "Output was kept pending because the attached DIBL could not be reapplied. \
     The last valid output and DIBL remain attached.";
pub const MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE: &str =
    "The output action was blocked because the attached DIBL could not be reapplied. \
     The existing output and DIBL remain attached.";
pub const MODEL_OUTPUT_STALE_MESSAGE: &str = "Device changed while output refinement was running.";
pub const MODEL_OUTPUT_SOURCE_NAME_MESSAGE: &str =
    "The output file has no visible file name and cannot be attached.";
pub const PARAMETER_FINITE_MESSAGE: &str = "Parameter must be a finite number.";
pub const SUBTHRESHOLD_POSITIVE_MESSAGE: &str = "SS and I_off must be positive numbers.";
pub const VDS_POSITIVE_MESSAGE: &str = "V_DS must be a positive number.";
pub const COX_NON_NEGATIVE_MESSAGE: &str = "Cox must be a non-negative number.";

pub fn analog_fit_row_status(
    gm_p90: Option<f64>,
    gds_p90: Option<f64>,
    has_output_curves: bool,
) -> Option<String> {
    match gm_p90 {
        None => Some("analog gm unqualified".to_string()),
        Some(error) if error >= ANALOG_GM_ERROR_LIMIT => {
            Some(format!("analog gm error {:.0}%", 100.0 * error))
        }
        _ => match gds_p90 {
            None if has_output_curves => Some("analog gds unqualified".to_string()),
            Some(error) if error >= ANALOG_GDS_ERROR_LIMIT => {
                Some(format!("analog gds error {:.0}%", 100.0 * error))
            }
            _ => None,
        },
    }
}

pub fn analog_fit_warning_message(
    gm_p90: Option<f64>,
    gds_p90: Option<f64>,
    has_output_curves: bool,
) -> Option<String> {
    match gm_p90 {
        None => Some(
            "Analog gm could not be qualified; do not trust this fit for analog simulation."
                .to_string(),
        ),
        Some(error) if error >= ANALOG_GM_ERROR_LIMIT => Some(format!(
            "Analog gm error is {:.0}%; review the fit before simulation.",
            100.0 * error
        )),
        _ => match gds_p90 {
            None if has_output_curves => Some(
                "Analog gds could not be qualified; do not trust output resistance.".to_string(),
            ),
            Some(error) if error >= ANALOG_GDS_ERROR_LIMIT => Some(format!(
                "Analog gds error is {:.0}%; review output resistance before simulation.",
                100.0 * error
            )),
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_fit_load_summary_surfaces_the_first_error_only_on_total_failure() {
        let total_fail = model_fit_loaded_devices_summary(0, 0, 0, 2, Some("no Vg column found"));
        assert!(
            total_fail.contains("First error: no Vg column found"),
            "{total_fail}"
        );

        let mixed = model_fit_loaded_devices_summary(1, 0, 0, 1, Some("no Vg column found"));
        assert!(!mixed.contains("First error"), "{mixed}");

        let clean = model_fit_loaded_devices_summary(2, 0, 0, 0, None);
        assert!(!clean.contains("First error"), "{clean}");

        assert_eq!(
            model_fit_loaded_devices_summary(1, 2, 3, 4, None),
            "Loaded 1 device(s); 2 already loaded, 3 unfittable, 4 parse error(s)."
        );
    }

    #[test]
    fn refinement_failure_summary_distinguishes_recovered_payloads() {
        assert_eq!(
            model_fit_output_refinement_failure("Output refinement", 2),
            "Output refinement failed unexpectedly. Kept 2 parsed output measurement(s) pending; \
             loaded device data was unchanged."
        );
        assert_eq!(
            model_fit_dibl_refinement_failure("DIBL detachment", 0),
            "DIBL detachment failed unexpectedly. Loaded device data was unchanged."
        );
    }

    #[test]
    fn analog_status_uses_committed_thresholds_and_gm_precedence() {
        assert_eq!(analog_fit_row_status(Some(0.149), None, false), None);
        assert_eq!(
            analog_fit_row_status(Some(0.15), Some(0.9), true).as_deref(),
            Some("analog gm error 15%")
        );
        assert_eq!(
            analog_fit_row_status(Some(0.1), Some(0.25), true).as_deref(),
            Some("analog gds error 25%")
        );
        assert_eq!(analog_fit_row_status(Some(0.1), None, false), None);
    }

    #[test]
    fn missing_derivatives_are_explicitly_unqualified() {
        assert_eq!(
            analog_fit_warning_message(None, None, false).as_deref(),
            Some("Analog gm could not be qualified; do not trust this fit for analog simulation.")
        );
        assert_eq!(
            analog_fit_warning_message(Some(0.1), None, true).as_deref(),
            Some("Analog gds could not be qualified; do not trust output resistance.")
        );
    }
}
