//! GUI-only display-formatting facade.
//!
//! The implementation is split by interface: reusable row labels/messages and
//! numeric engineering notation live behind this module.

mod labels;
mod numeric;

pub use labels::{
    already_loaded, analog_fit_row_status, analog_fit_warning_message, cleared_error_rows,
    cleared_model_fit_rows, exported_to, global_wl_message, loaded_files,
    model_fit_dibl_refinement_failure, model_fit_loaded_devices_summary,
    model_fit_output_refinement_failure, model_fit_output_summary, output_partial_fit_message,
    point_count_label, removed_items, status_badge, transfer_output_summary,
    ANALOG_GDS_ERROR_LIMIT, ANALOG_GM_ERROR_LIMIT, ATTACHED_PENDING_OUTPUT_MESSAGE,
    COX_NON_NEGATIVE_MESSAGE, LEVEL62_EXTRACTION_FAILED_MESSAGE, LOW_R2_MESSAGE, LOW_R2_THRESHOLD,
    MODEL_CV_STALE_MESSAGE, MODEL_DEVICE_REQUIRED_MESSAGE, MODEL_DIBL_AMBIGUOUS_MESSAGE,
    MODEL_DIBL_NO_MATCH_MESSAGE, MODEL_DIBL_REAPPLY_FAILED_MESSAGE, MODEL_DIBL_SOURCE_NAME_MESSAGE,
    MODEL_DIBL_STALE_MESSAGE, MODEL_OUTPUT_CLEAR_DIBL_CONFLICT_MESSAGE,
    MODEL_OUTPUT_DIBL_CONFLICT_MESSAGE, MODEL_OUTPUT_SOURCE_NAME_MESSAGE,
    MODEL_OUTPUT_STALE_MESSAGE, MODEL_PARAMETER_INVALID_MESSAGE, MODEL_REFIT_FAILED_MESSAGE,
    MODEL_SETUP_STALE_MESSAGE, OUTPUT_FIT_FAILED_MESSAGE, OUTPUT_MOVED_TO_PENDING_MESSAGE,
    OUTPUT_NO_FINITE_POINTS_MESSAGE, OUTPUT_SUMMARY_UNAVAILABLE_MESSAGE, PARAMETER_FINITE_MESSAGE,
    REMOVED_OUTPUT_MESSAGE, REMOVED_PENDING_OUTPUT_MESSAGE, SUBTHRESHOLD_POSITIVE_MESSAGE,
    VDS_POSITIVE_MESSAGE, WL_MODEL_INCOMPATIBLE_MESSAGE, WL_NUMERIC_MESSAGE, WL_POSITIVE_MESSAGE,
};
pub use numeric::{
    eng_tick, fmt_compact_current, fmt_current, fmt_eng, fmt_fixed2, fmt_num3, fmt_ohm, fmt_r2,
    fmt_ratio, fmt_slope, fmt_vg, parse_eng, DASH,
};
