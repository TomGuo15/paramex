//! The selected-device right rail: PARAMETERS shows the selected
//! device's full parameter set, top to bottom — compact channel/fit metadata,
//! editable model parameters, then device setup. The fitted rows follow the active
//! compact model; AOSTFT output-curve params show their extracted value once Id-Vd
//! curves are loaded.

use eframe::egui;
use egui_notify::Toasts;
#[cfg(test)]
use paramex_core::modelfit::FitModel;
use paramex_core::modelfit::{
    AboveThresholdFit, AnalogFitQuality, BiasParams, EditError, GeometryParams, InputError,
    Level62Fit, Level62Params, OutputParams, Polarity, SubthresholdParams,
};

use crate::format_ui::{
    analog_fit_warning_message, fmt_eng, fmt_num3, parse_eng, COX_NON_NEGATIVE_MESSAGE, DASH,
    LEVEL62_EXTRACTION_FAILED_MESSAGE, LOW_R2_MESSAGE, LOW_R2_THRESHOLD,
    MODEL_DEVICE_REQUIRED_MESSAGE, MODEL_DIBL_REAPPLY_FAILED_MESSAGE,
    MODEL_PARAMETER_INVALID_MESSAGE, OUTPUT_FIT_FAILED_MESSAGE, PARAMETER_FINITE_MESSAGE,
    SUBTHRESHOLD_POSITIVE_MESSAGE, VDS_POSITIVE_MESSAGE, WL_POSITIVE_MESSAGE,
};
use crate::io_tasks::IoQueue;
use crate::state::EditBuffers;
use crate::table_kit;
use crate::theme::tokens;
use crate::ui_kit::{self, Variant};
use crate::workspaces::modelfit::ingest::{start_export_card, start_setup_refinement, Msg};
use crate::workspaces::modelfit::state::{ModelFitState, SelectedMutationError, SetupOperation};
use crate::workspaces::modelfit::ModelFitWorkspace;

/// An owned copy of the selected device's display data, so the immutable borrow
/// of `state` ends before we (possibly) mutate the device's geometry.
struct DeviceSnapshot {
    /// Whether the active compact model is Level 62, so the read-only block and
    /// export action builds the right model's card from one snapshot.
    active_model_is_level62: bool,
    fit: AboveThresholdFit,
    output: Option<OutputParams>,
    subthreshold: Option<SubthresholdParams>,
    geometry: GeometryParams,
    bias: BiasParams,
    polarity: Polarity,
    has_output: bool,
    /// Whether raw Id-Vd sub-sweeps were loaded at all (independent of whether the output
    /// extraction succeeded) — lets the status line tell "no curves loaded" apart from
    /// "curves loaded but the fit failed".
    has_output_curves: bool,
    /// The stabilized Level 62-derived (LTPS / poly-Si) fit (when extraction succeeded), for
    /// the Level 62 display/export path.
    level62: Option<Level62Fit>,
    analog: AnalogFitQuality,
}

/// Group rail plus the two paired input rows. Keeping this block fixed lets the
/// terminal input sit against the card bottom while the longer model form scrolls.
const DEVICE_SETUP_BLOCK_HEIGHT: f32 = 124.0;

/// PARAMETERS: the selected device's full parameter set in one card — compact
/// channel/R² context, editable model values, then editable W/L/V_DS/Cox device
/// setup. The read-only set follows the active model (AOSTFT / Level 62). Inputs
/// commit on focus-loss via the shared changed-text gate.
pub fn show_parameters(
    ui: &mut egui::Ui,
    workspace: &mut ModelFitWorkspace,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
) {
    let ModelFitWorkspace { state, io, .. } = workspace;
    // Fill the right column. The model-form viewport scrolls independently so
    // the editable W/L/V_DS/Cox rows remain fixed at the card bottom.
    ui_kit::card_slot(ui, |ui| {
        let snap = device_snapshot(state);
        let ctx = ui.ctx().clone();
        let active_model_is_level62 = state.selected_model_is_level62();
        let active_model = state.selected_fit_model();
        let manual = snap
            .as_ref()
            .is_some_and(|_| state.is_selected_manual(active_model));
        let io_idle = io.is_idle();
        let mut setup_started = false;
        let (export_clicked, reset_clicked) = ui_kit::header_action_row(ui, "PARAMETERS", |ui| {
            let export_clicked = ui
                .add_enabled_ui(can_export_selected(state) && io_idle, |ui| {
                    ui_kit::header_action(ui, "Export Verilog-A", Variant::Primary).clicked()
                })
                .inner;
            let reset_clicked = ui
                .add_enabled_ui(manual && io_idle, |ui| {
                    ui_kit::header_action(ui, "Reset to Auto", Variant::Secondary).clicked()
                })
                .inner;
            (export_clicked, reset_clicked)
        });
        if reset_clicked {
            edits.forget_prefix("modelfit:p:");
            setup_started =
                launch_setup(&ctx, io, state, SetupOperation::Reset(active_model), toasts);
        }
        let body_h = ui.available_height().max(0.0);
        let polarity = snap.as_ref().map(|snap| snap.polarity);
        let r2 = snap.as_ref().and_then(|snap| {
            if snap.active_model_is_level62 {
                snap.level62.as_ref().map(|fit| fit.r2)
            } else {
                Some(snap.fit.r2)
            }
        });
        let width = ui.available_width();
        let (body_rect, _) =
            ui.allocate_exact_size(egui::vec2(width, body_h), egui::Sense::hover());
        let setup_top = (body_rect.bottom() - DEVICE_SETUP_BLOCK_HEIGHT).max(body_rect.top());
        let setup_rect =
            egui::Rect::from_min_max(egui::pos2(body_rect.left(), setup_top), body_rect.max);
        let model_bottom = (setup_rect.top() - ui.spacing().item_spacing.y).max(body_rect.top());
        let model_rect =
            egui::Rect::from_min_max(body_rect.min, egui::pos2(body_rect.right(), model_bottom));

        // Render in visual order so keyboard and accessibility traversal also
        // move from the top model form to the bottom device setup.
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(model_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_width(width);
                param_context_row(ui, polarity, r2);
                parameter_group_label(ui, "Model parameters");
                let model_form_height = ui.available_height();
                ui_kit::scroll_body(
                    ui,
                    "modelfit_model_parameters_body",
                    model_form_height,
                    |ui| {
                        if let Some(snap) = snap.as_ref() {
                            ui.add_enabled_ui(io.is_idle(), |ui| {
                                fitted_param_block(ui, state, edits, toasts, snap);
                            });
                            if !snap.active_model_is_level62 {
                                status_line(ui, snap);
                            }
                        } else {
                            disabled_model_form(ui, active_model_is_level62);
                        }
                    },
                );
            },
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(setup_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.set_width(width);
                parameter_group_label(ui, "Device setup");
                if let Some(snap) = snap.as_ref() {
                    ui.add_enabled_ui(io.is_idle(), |ui| {
                        device_input_rows(ui, state, io, edits, toasts, snap, &mut setup_started);
                    });
                } else {
                    disabled_pair(ui, "W (µm)", "L (µm)");
                    disabled_pair(ui, "V<sub>DS</sub> (V)", "C<sub>ox</sub> (nF/cm²)");
                }
            },
        );
        if export_clicked && !setup_started && io.is_idle() {
            if let Some((bytes, default_name)) = selected_model_card(state) {
                start_export_card(&ctx, io, bytes, default_name);
            }
        }
    });
}

/// The read-only fitted-parameter table for `snap`, dispatched by the active model.
/// Shared by [`show_parameters`]; no card frame of its own (it is composed into the
/// one flat PARAMETERS card).
fn fitted_param_block(
    ui: &mut egui::Ui,
    state: &mut ModelFitState,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    snap: &DeviceSnapshot,
) {
    let level62 = snap.active_model_is_level62;
    // Every model's parameter set is hand-editable (commit on focus-loss); a manual edit enables
    // the stable Reset to Auto action. Level 62 uses a data-driven form; AOSTFT spans three
    // structs, so it keeps its bespoke form.
    if level62 {
        if let Some(fit) = &snap.level62 {
            render_level62_param_form(
                ui,
                edits,
                toasts,
                state,
                fit.params,
                &level62_fields(),
                snap,
            );
        } else {
            disabled_level62_form(ui);
        }
    } else {
        aostft_param_inputs(ui, state, edits, toasts, snap);
    }
}

/// Build the owned display snapshot of the selected device (was inline in `show`).
fn device_snapshot(state: &ModelFitState) -> Option<DeviceSnapshot> {
    let entry = state.selected_entry()?;
    let device = entry.device();
    let model = device.model(state.selected_fit_model());
    Some(DeviceSnapshot {
        active_model_is_level62: state.selected_model_is_level62(),
        fit: *device.aostft_fit(),
        output: device.output(),
        subthreshold: device.subthreshold(),
        geometry: device.geometry(),
        bias: device.bias(),
        polarity: device.polarity(),
        has_output: device.has_output(),
        has_output_curves: device.has_output_curves(),
        level62: device.level62().cloned(),
        analog: model.analog_fit_quality(),
    })
}

fn disabled_model_form(ui: &mut egui::Ui, level62: bool) {
    if level62 {
        disabled_level62_form(ui);
    } else {
        disabled_pair(ui, "V<sub>TH</sub> (V)", "\u{03B3}");
        disabled_pair(ui, "K", "\u{03B1}<sub>sat</sub>");
        disabled_pair(ui, "\u{03BB} (1/V)", "m");
        disabled_pair(ui, "SS (mV/dec)", "I<sub>off</sub> (A)");
    }
}

fn disabled_level62_form(ui: &mut egui::Ui) {
    let fields = level62_fields();
    disabled_param_fields(ui, &fields, false);
    ui.add_enabled_ui(false, |ui| {
        egui::CollapsingHeader::new("Advanced / constants")
            .id_salt("modelfit_adv_level62")
            .show_unindented(ui, |ui| disabled_param_fields(ui, &fields, true));
    });
}

fn disabled_param_fields(ui: &mut egui::Ui, fields: &[Field<Level62Params>], advanced: bool) {
    let rows: Vec<_> = fields.iter().filter(|f| f.advanced == advanced).collect();
    for pair in rows.chunks(2) {
        if let Some(right) = pair.get(1) {
            disabled_pair(ui, pair[0].label, right.label);
        } else {
            let mut value = DASH.to_string();
            let width = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(width, ui_kit::PAIRED_SETTINGS_ROW_HEIGHT),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.add_enabled_ui(false, |ui| {
                        ui_kit::settings_cell(ui, pair[0].label, &mut value, width);
                    });
                },
            );
        }
    }
}

fn disabled_pair(ui: &mut egui::Ui, left_label: &str, right_label: &str) {
    let mut left = DASH.to_string();
    let mut right = DASH.to_string();
    ui.add_enabled_ui(false, |ui| {
        ui_kit::paired_settings_row(ui, left_label, &mut left, right_label, &mut right);
    });
}

fn can_export_selected(state: &ModelFitState) -> bool {
    state.selected_entry().is_some_and(|entry| {
        entry
            .device()
            .model(state.selected_fit_model())
            .is_export_ready()
    })
}

fn selected_model_card(state: &ModelFitState) -> Option<(Vec<u8>, String)> {
    let device = state.selected_entry()?.device();
    let artifact = device.model(state.selected_fit_model()).export_artifact()?;
    Some((artifact.text.into_bytes(), artifact.suggested_file_name))
}

/// AOSTFT parameter inputs: channel + the live overlay R² (read-only), then the full
/// editable card — above-threshold VT/γ/K, output coeffs α_sat/λ/m, and off-state SS/Ioff.
/// Each field commits on focus-loss via the shared changed-text gate (re-keyed under
/// `modelfit:p:aostft:` so a device switch drops a pending buffer), rejects non-finite input
/// with a toast, and on commit routes the overlay / R² / export to the edited value — putting
/// the device in AOSTFT manual mode (the header shows Reset to Auto). Output/off-state fields show
/// their effective value (the card default until refined), so editing one overrides from there.
fn aostft_param_inputs(
    ui: &mut egui::Ui,
    state: &mut ModelFitState,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    snap: &DeviceSnapshot,
) {
    let f = snap.fit;

    let (vt, gamma) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:p:aostft:vt",
        "V<sub>TH</sub> (V)",
        &fmt_num3(f.vt),
        "modelfit:p:aostft:gamma",
        "\u{03B3}",
        &fmt_num3(f.gamma),
    );
    if let Some(t) = vt {
        commit_fit(state, toasts, t, FitField::Vt);
    }
    if let Some(t) = gamma {
        commit_fit(state, toasts, t, FitField::Gamma);
    }

    let o = snap.output.unwrap_or_else(OutputParams::card_defaults);
    let (k, asat) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:p:aostft:k",
        "K",
        &fmt_eng(f.k),
        "modelfit:p:aostft:asat",
        "\u{03B1}<sub>sat</sub>",
        &fmt_num3(o.alpha_sat),
    );
    if let Some(t) = k {
        commit_fit(state, toasts, t, FitField::K);
    }
    if let Some(t) = asat {
        commit_output(state, toasts, t, OutField::AlphaSat);
    }
    let (lambda, m) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:p:aostft:lambda",
        "\u{03BB} (1/V)",
        &fmt_eng(o.lambda),
        "modelfit:p:aostft:m",
        "m",
        &fmt_num3(o.m),
    );
    if let Some(t) = lambda {
        commit_output(state, toasts, t, OutField::Lambda);
    }
    if let Some(t) = m {
        commit_output(state, toasts, t, OutField::M);
    }

    let s = snap
        .subthreshold
        .unwrap_or_else(SubthresholdParams::card_defaults);
    let (ss, ioff) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:p:aostft:ss",
        "SS (mV/dec)",
        &fmt_num3(s.ss_v_dec * 1000.0),
        "modelfit:p:aostft:ioff",
        "I<sub>off</sub> (A)",
        &fmt_eng(s.ioff),
    );
    if let Some(t) = ss {
        commit_subthreshold(state, toasts, t, SubField::Ss);
    }
    if let Some(t) = ioff {
        commit_subthreshold(state, toasts, t, SubField::Ioff);
    }
}

/// Which AOSTFT above-threshold field a committed value applies to.
#[derive(Clone, Copy)]
enum FitField {
    Vt,
    Gamma,
    K,
}

/// Parse one VT/γ/K field and override the selected device's AOSTFT fit (reading the other
/// two from current state so a single field edits in isolation). Warns on a non-finite value.
fn commit_fit(state: &mut ModelFitState, toasts: &mut Toasts, text: String, field: FitField) {
    let Some(f) = state
        .selected_entry()
        .map(|entry| *entry.device().aostft_fit())
    else {
        return;
    };
    match parse_eng(&text) {
        Some(v) => {
            let (mut vt, mut gamma, mut k) = (f.vt, f.gamma, f.k);
            match field {
                FitField::Vt => vt = v,
                FitField::Gamma => gamma = v,
                FitField::K => k = v,
            }
            if let Err(error) = state.set_selected_fit(vt, gamma, k) {
                warn_selected_mutation(toasts, error);
            }
        }
        _ => {
            toasts.warning(PARAMETER_FINITE_MESSAGE);
        }
    }
}

/// Which AOSTFT output-curve coefficient a committed value applies to.
#[derive(Clone, Copy)]
enum OutField {
    AlphaSat,
    Lambda,
    M,
}

/// Parse one α_sat/λ/m field and override the selected device's AOSTFT output coeffs (the
/// other two read from current state, defaulting when none were extracted). Warns on a
/// non-finite value.
fn commit_output(state: &mut ModelFitState, toasts: &mut Toasts, text: String, field: OutField) {
    let cur = state
        .selected_entry()
        .and_then(|entry| entry.device().output())
        .unwrap_or_else(OutputParams::card_defaults);
    match parse_eng(&text) {
        Some(v) => {
            let (mut a, mut l, mut m) = (cur.alpha_sat, cur.lambda, cur.m);
            match field {
                OutField::AlphaSat => a = v,
                OutField::Lambda => l = v,
                OutField::M => m = v,
            }
            if let Err(error) = state.set_selected_output(a, l, m) {
                warn_selected_mutation(toasts, error);
            }
        }
        _ => {
            toasts.warning(PARAMETER_FINITE_MESSAGE);
        }
    }
}

/// Which AOSTFT off-state field a committed value applies to.
#[derive(Clone, Copy)]
enum SubField {
    Ss,
    Ioff,
}

/// Parse one SS (mV/dec) / Ioff field and override the selected device's off-state params
/// (the other reads from current state, defaulting when none were extracted). Both must be
/// positive.
fn commit_subthreshold(
    state: &mut ModelFitState,
    toasts: &mut Toasts,
    text: String,
    field: SubField,
) {
    let cur = state
        .selected_entry()
        .and_then(|entry| entry.device().subthreshold())
        .unwrap_or_else(SubthresholdParams::card_defaults);
    match parse_eng(&text) {
        Some(v) if v > 0.0 => {
            let (mut ss, mut ioff) = (cur.ss_v_dec, cur.ioff);
            match field {
                SubField::Ss => ss = v / 1000.0, // displayed in mV/dec
                SubField::Ioff => ioff = v,
            }
            if let Err(error) = state.set_selected_subthreshold(ss, ioff) {
                warn_selected_mutation(toasts, error);
            }
        }
        _ => {
            toasts.warning(SUBTHRESHOLD_POSITIVE_MESSAGE);
        }
    }
}

/// One editable parameter-row descriptor for a single-`Params`-struct model. The data that
/// drives the generic form so each model is a small table, not a wall of hand-written rows.
struct Field<P: 'static> {
    /// Edit-buffer key suffix (unique per field within a model).
    key: &'static str,
    /// Display label (may carry `<sub>`/`<sup>` markup).
    label: &'static str,
    /// Value formatter: `fmt_num3` for O(1) values, `fmt_eng` for very small/large ones.
    fmt: fn(f64) -> String,
    get: fn(&P) -> f64,
    set: fn(&mut P, f64),
    /// `true` => rendered under the "Advanced / constants" collapsible, not inline.
    advanced: bool,
}

/// Render+commit one parameter input cell.
#[allow(clippy::too_many_arguments)]
fn param_cell<P: Copy>(
    ui: &mut egui::Ui,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    state: &mut ModelFitState,
    model_key: &str,
    cur: P,
    f: &Field<P>,
    apply: fn(&mut ModelFitState, P) -> Result<(), SelectedMutationError>,
    width: f32,
) {
    let key = format!("modelfit:p:{model_key}:{}", f.key);
    if let Some(t) =
        ui_kit::settings_cell_commit(ui, edits, &key, f.label, &(f.fmt)((f.get)(&cur)), width)
    {
        commit_param_text(state, toasts, cur, f, apply, t);
    }
}

fn commit_param_text<P: Copy>(
    state: &mut ModelFitState,
    toasts: &mut Toasts,
    cur: P,
    f: &Field<P>,
    apply: fn(&mut ModelFitState, P) -> Result<(), SelectedMutationError>,
    text: String,
) {
    match parse_eng(&text) {
        Some(v) => {
            let mut p = cur;
            (f.set)(&mut p, v);
            if let Err(error) = apply(state, p) {
                warn_selected_mutation(toasts, error);
            }
        }
        None => {
            toasts.warning(PARAMETER_FINITE_MESSAGE);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn param_pair<P: Copy>(
    ui: &mut egui::Ui,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    state: &mut ModelFitState,
    model_key: &str,
    cur: P,
    left: &Field<P>,
    right: Option<&Field<P>>,
    apply: fn(&mut ModelFitState, P) -> Result<(), SelectedMutationError>,
) {
    let row_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(row_w, ui_kit::PAIRED_SETTINGS_ROW_HEIGHT),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(row_w);
            ui.spacing_mut().item_spacing.y = ui_kit::INPUT_ROW_GAP;
            param_cell(ui, edits, toasts, state, model_key, cur, left, apply, row_w);
            if let Some(right) = right {
                param_cell(
                    ui, edits, toasts, state, model_key, cur, right, apply, row_w,
                );
            }
        },
    );
}

/// Level 62's primary fitted fields stay inline. Its warning occupies the same
/// row below the Advanced header whether that section is open or closed, so
/// expanding the constants cannot push an actionable fit warning off-screen.
#[allow(clippy::too_many_arguments)]
fn render_level62_param_form(
    ui: &mut egui::Ui,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    state: &mut ModelFitState,
    cur: Level62Params,
    fields: &[Field<Level62Params>],
    snap: &DeviceSnapshot,
) {
    render_param_fields(
        ui,
        edits,
        toasts,
        state,
        "level62",
        cur,
        fields,
        false,
        ModelFitState::set_selected_level62_params,
    );
    if fields.iter().any(|f| f.advanced) {
        let advanced = egui::CollapsingHeader::new("Advanced / constants")
            .id_salt("modelfit_adv_level62")
            .show_unindented(ui, |ui| {
                level62_status_line(ui, snap);
                render_param_fields(
                    ui,
                    edits,
                    toasts,
                    state,
                    "level62",
                    cur,
                    fields,
                    true,
                    ModelFitState::set_selected_level62_params,
                );
            });
        if advanced.body_returned.is_none() {
            level62_status_line(ui, snap);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_param_fields<P: Copy>(
    ui: &mut egui::Ui,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    state: &mut ModelFitState,
    model_key: &str,
    cur: P,
    fields: &[Field<P>],
    advanced: bool,
    apply: fn(&mut ModelFitState, P) -> Result<(), SelectedMutationError>,
) {
    let rows: Vec<_> = fields.iter().filter(|f| f.advanced == advanced).collect();
    for pair in rows.chunks(2) {
        param_pair(
            ui,
            edits,
            toasts,
            state,
            model_key,
            cur,
            pair[0],
            pair.get(1).copied(),
            apply,
        );
    }
}

/// One quiet context rail for channel and the live transfer-overlay R².
fn param_context_row(ui: &mut egui::Ui, polarity: Option<Polarity>, r2: Option<f64>) {
    // A non-finite R² (degenerate / zero-variance curve) reads as "—", not "NaN".
    let r2_cell = r2
        .map(|r2| {
            if r2.is_finite() {
                fmt_num3(r2)
            } else {
                DASH.into()
            }
        })
        .unwrap_or_else(|| DASH.into());
    let channel = polarity.map(channel_label).unwrap_or(DASH);
    let row_w = ui.available_width();
    let gap = ui_kit::INPUT_PAIR_GAP;
    let cell_w = ((row_w - gap) * 0.5).max(0.0);
    let (row, _) =
        ui.allocate_exact_size(egui::vec2(row_w, table_kit::ROW_H), egui::Sense::hover());
    let left = egui::Rect::from_min_size(row.min, egui::vec2(cell_w, row.height()));
    let right = egui::Rect::from_min_size(
        egui::pos2(left.right() + gap, row.top()),
        egui::vec2(cell_w, row.height()),
    );
    for (rect, label, value) in [
        (left, "channel", channel),
        (right, "transfer R<sup>2</sup>", r2_cell.as_str()),
    ] {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_width(rect.width());
                ui_kit::metric_label(ui, label);
                ui_kit::right_aligned(ui, |ui| {
                    ui_kit::metric_value(ui, value);
                });
            },
        );
    }
    ui.add_space(6.0);
}

fn parameter_group_label(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(
            egui::RichText::new(title)
                .size(10.5)
                .strong()
                .color(tokens().ink),
        );
        let (rule, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().hline(
            rule.x_range(),
            rule.center().y,
            egui::Stroke::new(1.0_f32, tokens().border),
        );
    });
    ui.add_space(2.0);
}

#[rustfmt::skip]
fn level62_fields() -> Vec<Field<Level62Params>> {
    vec![
        Field { key: "vto", label: "VTO (V)", fmt: fmt_num3, get: |p| p.vto, set: |p, v| p.vto = v, advanced: false },
        Field { key: "vfb", label: "VFB (V)", fmt: fmt_num3, get: |p| p.vfb, set: |p, v| p.vfb = v, advanced: false },
        // Mobilities are stored SI (m²/Vs) but SHOWN/entered in cm²/Vs (the TFT-community
        // convention): get ×1e4 (m²→cm²), set ×1e-4 back to SI so the core stays SI.
        Field { key: "mu0", label: "\u{03BC}<sub>0</sub> (cm\u{00B2}/Vs)", fmt: fmt_eng, get: |p| p.mu0 * 1.0e4, set: |p, v| p.mu0 = v * 1.0e-4, advanced: false },
        Field { key: "mu1", label: "\u{03BC}<sub>1</sub> (cm\u{00B2}/Vs)", fmt: fmt_eng, get: |p| p.mu1 * 1.0e4, set: |p, v| p.mu1 = v * 1.0e-4, advanced: false },
        Field { key: "mmu", label: "MMU", fmt: fmt_num3, get: |p| p.mmu, set: |p, v| p.mmu = v, advanced: false },
        Field { key: "mus", label: "\u{03BC}<sub>s</sub> (cm\u{00B2}/Vs)", fmt: fmt_eng, get: |p| p.mus * 1.0e4, set: |p, v| p.mus = v * 1.0e-4, advanced: false },
        Field { key: "asat", label: "ASAT", fmt: fmt_num3, get: |p| p.asat, set: |p, v| p.asat = v, advanced: false },
        Field { key: "lambda", label: "LAMBDA (1/V)", fmt: fmt_eng, get: |p| p.lambda, set: |p, v| p.lambda = v, advanced: false },
        Field { key: "eta", label: "ETA", fmt: fmt_num3, get: |p| p.eta, set: |p, v| p.eta = v, advanced: false },
        Field { key: "delta", label: "DELTA", fmt: fmt_num3, get: |p| p.delta, set: |p, v| p.delta = v, advanced: false },
        Field { key: "vkink", label: "VKINK (V)", fmt: fmt_num3, get: |p| p.vkink, set: |p, v| p.vkink = v, advanced: true },
        Field { key: "lkink", label: "LKINK (m)", fmt: fmt_eng, get: |p| p.lkink, set: |p, v| p.lkink = v, advanced: true },
        Field { key: "mk", label: "MK", fmt: fmt_num3, get: |p| p.mk, set: |p, v| p.mk = v, advanced: true },
        Field { key: "i00", label: "I00 (A/m)", fmt: fmt_eng, get: |p| p.i00, set: |p, v| p.i00 = v, advanced: true },
        Field { key: "eb", label: "EB (eV)", fmt: fmt_num3, get: |p| p.eb, set: |p, v| p.eb = v, advanced: true },
        Field { key: "eps", label: "EPS", fmt: fmt_num3, get: |p| p.eps, set: |p, v| p.eps = v, advanced: true },
        Field { key: "epsi", label: "EPSI", fmt: fmt_num3, get: |p| p.epsi, set: |p, v| p.epsi = v, advanced: true },
        Field { key: "tox", label: "TOX (m)", fmt: fmt_eng, get: |p| p.tox, set: |p, v| p.tox = v, advanced: true },
        Field { key: "rs", label: "RS (\u{03A9})", fmt: fmt_eng, get: |p| p.rs, set: |p, v| p.rs = v, advanced: true },
        Field { key: "rd", label: "RD (\u{03A9})", fmt: fmt_eng, get: |p| p.rd, set: |p, v| p.rd = v, advanced: true },
        // DIBL + temperature structure — carried at no-op defaults (AT =
        // BT = 0, zero temperature coefficients); hand-set them to dial known
        // second-order behavior into the overlay and the exported card.
        Field { key: "at", label: "AT (m/V)", fmt: fmt_eng, get: |p| p.at, set: |p, v| p.at = v, advanced: true },
        Field { key: "bt", label: "BT (m\u{00B7}V)", fmt: fmt_eng, get: |p| p.bt, set: |p, v| p.bt = v, advanced: true },
        Field { key: "vsi", label: "VSI (V)", fmt: fmt_num3, get: |p| p.vsi, set: |p, v| p.vsi = v, advanced: true },
        Field { key: "vst", label: "VST (V)", fmt: fmt_num3, get: |p| p.vst, set: |p, v| p.vst = v, advanced: true },
        Field { key: "dvto", label: "DVTO (V/K)", fmt: fmt_eng, get: |p| p.dvto, set: |p, v| p.dvto = v, advanced: true },
        // Same SI-storage / cm²-display convention as the mobilities above.
        Field { key: "dmu1", label: "DMU1 (cm\u{00B2}/Vs\u{00B7}K)", fmt: fmt_eng, get: |p| p.dmu1 * 1.0e4, set: |p, v| p.dmu1 = v * 1.0e-4, advanced: true },
        Field { key: "dasat", label: "DASAT (1/K)", fmt: fmt_eng, get: |p| p.dasat, set: |p, v| p.dasat = v, advanced: true },
        Field { key: "lasat", label: "LASAT (m)", fmt: fmt_eng, get: |p| p.lasat, set: |p, v| p.lasat = v, advanced: true },
    ]
}

/// Editable device geometry (W/L) and bias/process (V_DS, Cox). All fields commit
/// on focus-loss via the shared changed-text gate and re-key under `modelfit:geom:`
/// so a device switch drops any pending buffer (no cross-device commit). Invalid
/// entries are rejected with a toast. Rendered below the card's Device setup label.
fn device_input_rows(
    ui: &mut egui::Ui,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
    edits: &mut EditBuffers,
    toasts: &mut Toasts,
    snap: &DeviceSnapshot,
    setup_started: &mut bool,
) {
    let ctx = ui.ctx().clone();
    let geom = snap.geometry;
    let bias = snap.bias;
    let (w, l) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:geom:w",
        "W (\u{00B5}m)",
        &fmt_num3(geom.w_um),
        "modelfit:geom:l",
        "L (\u{00B5}m)",
        &fmt_num3(geom.l_um),
    );
    if w.is_some() || l.is_some() {
        commit_geometry(&ctx, state, io, toasts, w, l, setup_started);
    }
    // V_DS anchors the conductance (must be > 0); Cox drives the AC charge model
    // (0 = DC-only). Cox uses a round-tripping scientific display (it is tiny).
    let (vds, cox) = ui_kit::paired_settings_row_commit(
        ui,
        edits,
        "modelfit:geom:vds",
        "V<sub>DS</sub> (V)",
        &fmt_num3(bias.v_ds),
        "modelfit:geom:cox",
        "C<sub>ox</sub> (nF/cm\u{00B2})",
        &fmt_cox(bias.cox),
    );
    if let Some(text) = cox {
        commit_cox(state, io, toasts, text, *setup_started);
    }
    if let Some(text) = vds {
        commit_vds(&ctx, state, io, toasts, text, setup_started);
    }
}

fn commit_geometry(
    ctx: &egui::Context,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
    toasts: &mut Toasts,
    w: Option<String>,
    l: Option<String>,
    setup_started: &mut bool,
) {
    let mut geom = state
        .selected_entry()
        .map(|entry| entry.device().geometry())
        .unwrap_or_default();
    let parsed = |text: &str| {
        text.trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
    };
    if let Some(text) = w {
        let Some(value) = parsed(&text) else {
            toasts.warning(WL_POSITIVE_MESSAGE);
            return;
        };
        geom.w_um = value;
    }
    if let Some(text) = l {
        let Some(value) = parsed(&text) else {
            toasts.warning(WL_POSITIVE_MESSAGE);
            return;
        };
        geom.l_um = value;
    }
    if !*setup_started && io.is_idle() {
        *setup_started = launch_setup(ctx, io, state, SetupOperation::Geometry(geom), toasts);
    }
}

fn commit_cox(
    state: &mut ModelFitState,
    io: &IoQueue<Msg>,
    toasts: &mut Toasts,
    text: String,
    setup_started: bool,
) {
    let parsed = text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0);
    match parsed {
        Some(value) if !setup_started && io.is_idle() => {
            if let Err(error) = state.set_selected_cox(value * 1.0e-5) {
                warn_selected_mutation(toasts, error);
            }
        }
        Some(_) => {}
        None => {
            toasts.warning(COX_NON_NEGATIVE_MESSAGE);
        }
    }
}

fn commit_vds(
    ctx: &egui::Context,
    state: &mut ModelFitState,
    io: &mut IoQueue<Msg>,
    toasts: &mut Toasts,
    text: String,
    setup_started: &mut bool,
) {
    match text
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        Some(v_ds) if !*setup_started && io.is_idle() => {
            *setup_started = launch_setup(ctx, io, state, SetupOperation::DrainBias(v_ds), toasts);
        }
        Some(_) => {}
        None => {
            toasts.warning(VDS_POSITIVE_MESSAGE);
        }
    }
}

fn launch_setup(
    ctx: &egui::Context,
    io: &mut IoQueue<Msg>,
    state: &ModelFitState,
    operation: SetupOperation,
    toasts: &mut Toasts,
) -> bool {
    if !io.is_idle() {
        return false;
    }
    match state.plan_selected_setup(operation) {
        Ok(Some(plan)) => {
            start_setup_refinement(ctx, io, plan);
            true
        }
        Ok(None) => false,
        Err(error) => {
            warn_selected_mutation(toasts, error);
            false
        }
    }
}

/// A round-tripping display for the (tiny) Cox value: plain `0` when off, else
/// 4-figure scientific so the field re-seeds to exactly what it shows.
/// Format the stored `Cox` (SI, F/m²) for display in the conventional TFT unit
/// nF/cm² (`F/m² × 1e5`): e.g. `3.45e-4 F/m²` → `34.500`. `0` stays `"0"` (DC-only).
fn fmt_cox(cox_fm2: f64) -> String {
    if cox_fm2 == 0.0 {
        "0".to_string()
    } else {
        fmt_num3(cox_fm2 * 1.0e5)
    }
}

fn warn_selected_mutation(toasts: &mut Toasts, error: SelectedMutationError) {
    toasts.warning(selected_mutation_message(error));
}

fn selected_mutation_message(error: SelectedMutationError) -> &'static str {
    match error {
        SelectedMutationError::NoDeviceSelected => MODEL_DEVICE_REQUIRED_MESSAGE,
        SelectedMutationError::Input(InputError::InvalidGeometry) => WL_POSITIVE_MESSAGE,
        SelectedMutationError::Input(InputError::InvalidBias) => VDS_POSITIVE_MESSAGE,
        SelectedMutationError::Input(InputError::InvalidAccumulationCapacitance)
        | SelectedMutationError::Input(InputError::InvalidAostftCardMapping) => {
            MODEL_PARAMETER_INVALID_MESSAGE
        }
        SelectedMutationError::Input(InputError::RetainedDiblNotApplied) => {
            MODEL_DIBL_REAPPLY_FAILED_MESSAGE
        }
        SelectedMutationError::Edit(EditError::NonFiniteAostftFit) => PARAMETER_FINITE_MESSAGE,
        SelectedMutationError::Edit(EditError::InvalidSubthreshold) => {
            SUBTHRESHOLD_POSITIVE_MESSAGE
        }
        SelectedMutationError::Edit(
            EditError::InvalidAostftFit
            | EditError::InvalidOutput
            | EditError::InvalidLevel62Params
            | EditError::InvalidAostftCardMapping,
        ) => MODEL_PARAMETER_INVALID_MESSAGE,
        SelectedMutationError::Edit(EditError::NoLevel62Fit) => LEVEL62_EXTRACTION_FAILED_MESSAGE,
    }
}

/// Fit-status line. Output curves are OPTIONAL: a transfer-only device exports a
/// valid card with alpha_sat/lambda/m at labeled defaults; loading output curves
/// refines them. Successful fits stay silent here; the parameter rows and plots
/// already show the fitted state.
fn status_line(ui: &mut egui::Ui, snap: &DeviceSnapshot) {
    let warning = if snap.fit.r2 < LOW_R2_THRESHOLD {
        Some(LOW_R2_MESSAGE.to_string())
    } else if snap.has_output_curves && !snap.has_output {
        Some(OUTPUT_FIT_FAILED_MESSAGE.to_string())
    } else {
        analog_fit_warning_message(
            snap.analog.gm_p90,
            snap.analog.gds_p90,
            snap.has_output_curves,
        )
    };
    if let Some(warning) = warning {
        ui_kit::status_line(ui, &warning);
    }
}

/// Fit-status line for the Level 62 (LTPS) model.
fn level62_status_line(ui: &mut egui::Ui, snap: &DeviceSnapshot) {
    let warning = match snap.level62.as_ref().map(|fit| fit.r2) {
        None => Some(LEVEL62_EXTRACTION_FAILED_MESSAGE.to_string()),
        Some(r2) if r2 < LOW_R2_THRESHOLD => Some(LOW_R2_MESSAGE.to_string()),
        Some(_) => analog_fit_warning_message(
            snap.analog.gm_p90,
            snap.analog.gds_p90,
            snap.has_output_curves,
        ),
    };
    if let Some(warning) = warning {
        ui_kit::status_line(ui, &warning);
    }
}

/// The channel-type label for a device's detected polarity, shown as the first row
/// of every model's parameter table.
fn channel_label(polarity: Polarity) -> &'static str {
    match polarity {
        Polarity::NChannel => "n-channel",
        Polarity::PChannel => "p-channel",
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{
        kittest::{NodeT, Queryable},
        Harness,
    };

    use super::*;
    use crate::io_tasks::spawn_io;

    /// A state with the AOSTFT-shaped demo device selected (model index 0).
    fn demo_state() -> ModelFitState {
        let mut s = ModelFitState::default();
        s.load_demo(); // "demo: organic" selected, AOSTFT active
        s
    }

    struct BusyParametersApp {
        workspace: ModelFitWorkspace,
        edits: EditBuffers,
        toasts: Toasts,
    }

    impl eframe::App for BusyParametersApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            show_parameters(ui, &mut self.workspace, &mut self.edits, &mut self.toasts);
        }
    }

    #[test]
    fn actions_and_parameters_are_disabled_while_io_is_in_flight() {
        let mut state = demo_state();
        let fit = *state.selected_entry().unwrap().device().aostft_fit();
        assert!(state
            .set_selected_fit(fit.vt + 1.0, fit.gamma, fit.k)
            .is_ok());
        let fit_before = *state.selected_entry().unwrap().device().aostft_fit();
        let mut workspace = ModelFitWorkspace::from_state(state);
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        spawn_io(
            &egui::Context::default(),
            &mut workspace.io,
            "blocked Model Fit test worker",
            move || -> Option<Msg> {
                let _ = release_rx.recv();
                None
            },
        );
        let app = BusyParametersApp {
            workspace,
            edits: EditBuffers::default(),
            toasts: Toasts::default(),
        };
        let mut harness = Harness::builder()
            .with_size(egui::vec2(320.0, 800.0))
            .build_eframe(|cc| {
                crate::theme::install(&cc.egui_ctx);
                app
            });
        harness.run();

        for label in ["Export Verilog-A", "Reset to Auto"] {
            assert!(
                harness.get_by_label(label).accesskit_node().is_disabled(),
                "{label} should be disabled while Model Fit IO is running"
            );
        }
        let parameter_inputs = harness
            .get_all_by_role(egui::accesskit::Role::TextInput)
            .collect::<Vec<_>>();
        assert!(!parameter_inputs.is_empty());
        assert!(parameter_inputs
            .iter()
            .all(|input| input.accesskit_node().is_disabled()));

        parameter_inputs[0].click();
        harness.run();
        let selected = harness
            .state()
            .workspace
            .state()
            .selected_entry()
            .expect("selected demo device remains visible");
        assert_eq!(*selected.device().aostft_fit(), fit_before);

        release_tx.send(()).unwrap();
    }

    #[test]
    fn commit_fit_routes_one_field_and_enters_manual() {
        let mut s = demo_state();
        let mut toasts = Toasts::default();
        let f0 = *s.selected_entry().unwrap().device().aostft_fit();
        commit_fit(&mut s, &mut toasts, "3.5".into(), FitField::Vt);
        let f = *s.selected_entry().unwrap().device().aostft_fit();
        assert_eq!(f.vt, 3.5, "VT routed from the committed text");
        assert_eq!(f.gamma, f0.gamma, "only VT changed");
        assert_eq!(f.k, f0.k, "only VT changed");
        assert!(
            s.is_selected_manual(FitModel::Aostft),
            "an edit enters manual mode"
        );
    }

    #[test]
    fn commit_fit_rejects_non_numeric_and_stays_auto() {
        let mut s = demo_state();
        let mut toasts = Toasts::default();
        let vt0 = s.selected_entry().unwrap().device().aostft_fit().vt;
        commit_fit(&mut s, &mut toasts, "not a number".into(), FitField::Vt);
        assert_eq!(
            s.selected_entry().unwrap().device().aostft_fit().vt,
            vt0,
            "garbage left the value unchanged"
        );
        assert!(
            !s.is_selected_manual(FitModel::Aostft),
            "a rejected edit does not enter manual mode"
        );
    }

    #[test]
    fn commit_subthreshold_converts_mv_per_dec_to_v_per_dec() {
        let mut s = demo_state();
        let mut toasts = Toasts::default();
        commit_subthreshold(&mut s, &mut toasts, "300".into(), SubField::Ss);
        let ss = s
            .selected_entry()
            .unwrap()
            .device()
            .subthreshold()
            .expect("subthreshold set")
            .ss_v_dec;
        assert!(
            (ss - 0.3).abs() < 1e-9,
            "300 mV/dec commits as 0.3 V/dec: {ss}"
        );
    }

    #[test]
    fn commit_output_routes_one_coefficient() {
        let mut s = demo_state();
        let mut toasts = Toasts::default();
        commit_output(&mut s, &mut toasts, "2.7".into(), OutField::M);
        assert_eq!(
            s.selected_entry()
                .unwrap()
                .device()
                .output()
                .expect("output set")
                .m,
            2.7
        );
        assert!(s.is_selected_manual(FitModel::Aostft));
    }

    #[test]
    fn parse_valid_nonphysical_edit_surfaces_the_core_rejection() {
        let mut s = demo_state();
        let mut toasts = Toasts::default();
        let before = s.selected_entry().unwrap().device().output().unwrap();

        commit_output(&mut s, &mut toasts, "-1".into(), OutField::AlphaSat);

        assert_eq!(
            s.selected_entry().unwrap().device().output(),
            Some(before),
            "the rejected parameter combination must not mutate the fitted device"
        );
        assert_eq!(
            selected_mutation_message(SelectedMutationError::Edit(EditError::InvalidOutput)),
            MODEL_PARAMETER_INVALID_MESSAGE
        );
        assert!(
            !s.is_selected_manual(FitModel::Aostft),
            "a rejected edit must not enter manual mode"
        );
    }

    #[test]
    fn retained_dibl_mutation_rejection_explains_the_required_resolution() {
        assert_eq!(
            selected_mutation_message(SelectedMutationError::Input(
                InputError::RetainedDiblNotApplied
            )),
            MODEL_DIBL_REAPPLY_FAILED_MESSAGE
        );
    }

    #[test]
    fn selected_model_card_delegates_the_core_export_artifact() {
        let s = demo_state();
        let expected = s
            .selected_entry()
            .unwrap()
            .device()
            .model(s.selected_fit_model())
            .export_artifact()
            .expect("core artifact");
        let (bytes, default_name) = selected_model_card(&s).expect("card");
        assert_eq!(default_name, expected.suggested_file_name);
        assert_eq!(bytes, expected.text.into_bytes());
    }

    #[test]
    fn model_specific_field_labels_carry_the_export_units() {
        let level62 = level62_fields();
        assert_eq!(
            level62.iter().find(|f| f.key == "lkink").unwrap().label,
            "LKINK (m)"
        );
        assert_eq!(
            level62.iter().find(|f| f.key == "i00").unwrap().label,
            "I00 (A/m)"
        );
        assert_eq!(
            level62.iter().find(|f| f.key == "eb").unwrap().label,
            "EB (eV)"
        );
    }

    #[test]
    fn level62_primary_parameter_grid_has_no_lone_row() {
        let fields = level62_fields();
        let primary: Vec<_> = fields.iter().filter(|f| !f.advanced).collect();
        assert_eq!(
            primary.len() % 2,
            0,
            "Level 62 primary PARAMETERS fields should render as complete pairs"
        );
        let eta_pair = primary
            .chunks(2)
            .find(|pair| pair.iter().any(|f| f.key == "eta"))
            .expect("ETA stays in the primary grid");
        assert_eq!(
            eta_pair.len(),
            2,
            "ETA should have a visible row mate in the primary grid"
        );
    }
}
