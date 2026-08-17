use crate::common::{crate_file, visit_rs_files};
use paramex_gui::format_ui::{
    already_loaded, cleared_error_rows, cleared_model_fit_rows, global_wl_message,
    model_fit_loaded_devices_summary, model_fit_output_summary, point_count_label, removed_items,
    status_badge, transfer_output_summary, ATTACHED_PENDING_OUTPUT_MESSAGE,
    COX_NON_NEGATIVE_MESSAGE, DASH, LEVEL62_EXTRACTION_FAILED_MESSAGE, OUTPUT_FIT_FAILED_MESSAGE,
    OUTPUT_MOVED_TO_PENDING_MESSAGE, PARAMETER_FINITE_MESSAGE, REMOVED_OUTPUT_MESSAGE,
    REMOVED_PENDING_OUTPUT_MESSAGE, SUBTHRESHOLD_POSITIVE_MESSAGE, VDS_POSITIVE_MESSAGE,
    WL_NUMERIC_MESSAGE,
};

#[test]
fn format_ui_facade_exports_label_and_numeric_contracts() {
    assert_eq!(point_count_label(1), "1 pts");
    assert_eq!(point_count_label(2), "2 pts");
    assert_eq!(status_badge(false), "ok");
    assert_eq!(status_badge(true), "error");
    assert_eq!(global_wl_message(2), "Applied W/L to 2 file(s).");
    assert_eq!(already_loaded("a.csv"), "a.csv is already loaded.");
    assert_eq!(removed_items(2, "file"), "Removed 2 file(s).");
    assert_eq!(cleared_error_rows(), "Cleared error row(s).");
    assert_eq!(
        cleared_model_fit_rows(2, 3, 1),
        "Cleared 2 device(s), 3 pending measurement row(s), and 1 error row(s)."
    );
    assert_eq!(
        model_fit_loaded_devices_summary(0, 0, 0, 1, Some("bad header")),
        "Loaded 0 device(s); 0 already loaded, 0 unfittable, 1 parse error(s). First error: bad header"
    );
    assert_eq!(
        transfer_output_summary(1, 2, 3, 4, 5, None),
        "Transfer output: 1 attached, 2 unmatched, 3 ambiguous, 4 displaced, 5 error(s)."
    );
    assert_eq!(
        model_fit_output_summary(0, 0, 0, 1, 0, 1, Some("bad output")),
        "Output curves: 0 attached, 0 unmatched, 0 ambiguous, 1 displaced, 0 unfittable, 1 error(s). First error: bad output"
    );
    assert_eq!(WL_NUMERIC_MESSAGE, "W and L must be numeric.");
    assert_eq!(OUTPUT_FIT_FAILED_MESSAGE, "Output fit failed.");
    assert_eq!(ATTACHED_PENDING_OUTPUT_MESSAGE, "Attached pending output.");
    assert_eq!(REMOVED_PENDING_OUTPUT_MESSAGE, "Removed pending output.");
    assert_eq!(OUTPUT_MOVED_TO_PENDING_MESSAGE, "Output moved to pending.");
    assert_eq!(REMOVED_OUTPUT_MESSAGE, "Removed output.");
    assert_eq!(
        LEVEL62_EXTRACTION_FAILED_MESSAGE,
        "Level 62 extraction failed."
    );
    assert_eq!(
        PARAMETER_FINITE_MESSAGE,
        "Parameter must be a finite number."
    );
    assert_eq!(
        SUBTHRESHOLD_POSITIVE_MESSAGE,
        "SS and I_off must be positive numbers."
    );
    assert_eq!(VDS_POSITIVE_MESSAGE, "V_DS must be a positive number.");
    assert_eq!(
        COX_NON_NEGATIVE_MESSAGE,
        "Cox must be a non-negative number."
    );
    assert_eq!(DASH, "\u{2014}");
}

#[test]
fn shared_ui_copy_is_not_duplicated_outside_format_ui() {
    let needles = [
        "W and L must be numeric.",
        "Output fit failed.",
        "Attached pending output.",
        "Removed pending output.",
        "Output moved to pending.",
        "Removed output.",
        "Level 62 extraction failed.",
        "Parameter must be a finite number.",
        "SS and I_off must be positive numbers.",
        "V_DS must be a positive number.",
        "Cox must be a non-negative number.",
        "Cleared error row(s).",
        "Loaded {fitted} device(s);",
        "Transfer output: {attached} attached,",
        "Output curves: {attached} attached,",
        "Removed {count} {noun}(s).",
    ];
    let mut violations = Vec::new();

    visit_rs_files(crate_file("src"), |path, source| {
        let normalized = path.to_string_lossy().replace('\\', "/");
        if normalized.ends_with("/src/format_ui.rs") || normalized.contains("/src/format_ui/") {
            return;
        }
        for (line_idx, line) in source.lines().enumerate() {
            for needle in needles {
                if line.contains(needle) {
                    violations.push(format!(
                        "{}:{} duplicates shared UI copy `{needle}`",
                        path.display(),
                        line_idx + 1
                    ));
                }
            }
        }
    });

    assert!(
        violations.is_empty(),
        "shared user-facing copy belongs in format_ui, independent of caller layout:\n{}",
        violations.join("\n")
    );
}
