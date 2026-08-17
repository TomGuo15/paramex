//! OUTPUT plot: the selected device's Id-Vd family on LINEAR axes, the way
//! an output characteristic is conventionally read. Two modes:
//!
//! - **Measured** (Id-Vd curves were loaded): the measured family (solid, one line
//!   per gate sub-sweep) with the device's modelled overlay dashed on top — a true
//!   measured-vs-exported-model fit check.
//! - **Predicted** (transfer-only device, no measured output): four gate steps of
//!   the model's own Id-Vd family (solid), computed from the transfer fit. This is
//!   exactly what the exported card produces — real drive-current levels from `K`,
//!   with the saturation shape at the card defaults until Id-Vd curves are loaded.
//!
//! Both modes keep their samples in the on-direction first quadrant internally,
//! then project the V_D labels and tooltips back to the device frame. A p-channel
//! axis therefore reads 0, -V_D from left to right while |I_D| stays positive.
//! Gate voltage is encoded as a Studio-Stellar blue ramp (pale = low drive, vivid
//! = high drive).

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, LineStyle, PlotBounds, PlotPoints};
use paramex_core::modelfit::OutputSeries;

use crate::format_ui::{eng_tick, fmt_num3, OUTPUT_FIT_FAILED_MESSAGE};
use crate::plot_kit::{self, LegendMark};
use crate::ui_kit::{self, BadgeTone, StatusLineText};
use crate::workspaces::modelfit::state::ModelFitState;

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        // Measured family when Id-Vd curves were loaded, else the predicted family
        // from the transfer fit. The overlay follows the active model.
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let (series, measured_mode, sign) = if let Some(entry) = selected {
            let device = entry.device();
            (
                device.model(state.selected_fit_model()).output_family(),
                device.has_output_curves(),
                device.polarity().sign(),
            )
        } else {
            (Vec::new(), false, 1.0)
        };
        let has_measured = series.iter().any(|s| !s.measured.is_empty());
        let has_model = series.iter().any(|s| !s.modelled.is_empty());
        let fit_failed = measured_mode && !has_model;

        // Linear bounds anchored at the origin. Scale to the MEASURED points where
        // they exist (so a mis-set V_DS that inflates the model overlay can't squash
        // the real family — the overlay just clips past the top); predicted curves
        // have no measured points, so they set the scale themselves.
        let (mut xmax, mut ymax) = (0.0_f64, 0.0_f64);
        for s in &series {
            let pts = if s.measured.is_empty() {
                &s.modelled
            } else {
                &s.measured
            };
            for p in pts {
                xmax = xmax.max(p[0]);
                ymax = ymax.max(p[1]);
            }
        }
        if !xmax.is_finite() || xmax <= 0.0 {
            xmax = 1.0;
        }
        if !ymax.is_finite() || ymax <= 0.0 {
            ymax = 1.0;
        }
        let (xhi, yhi) = (xmax * 1.04, ymax * 1.06);

        // Order the gate sub-sweeps by on-direction drive so the color ramp tracks
        // increasing V_G regardless of file row order.
        let order = super::gate_drive_order(&series, sign);
        let gate_metadata = super::gate_drive_metadata(&series, &order);
        let n = order.len().max(1);

        ui_kit::section_header(ui, "OUTPUT FIT", gate_metadata.as_deref());

        let base = egui::TextStyle::Body.resolve(ui.style());
        let xstep = plot_kit::nice_axis_step(xhi);
        let ystep = plot_kit::nice_axis_step(yhi);
        let x_hints = plot_kit::muted_axis(Axis::X, "V<sub>D</sub> (V)", base.clone())
            .formatter(move |mark: GridMark, _range| {
                plot_kit::numeric_tick_label(has_device, sign * mark.value)
            })
            .label_spacing(egui::Rangef::new(24.0, 40.0));
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(Axis::Y, "|I<sub>D</sub>| (A)", base)
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::engineering_tick_label(has_device, mark.value)
                })
                .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );

        super::plot_grid(ui, "modelfit_output_plot")
            .custom_x_axes(vec![x_hints])
            .custom_y_axes(vec![y_hints])
            .x_grid_spacer(move |_input| plot_kit::grid_marks(0.0, xhi, xstep))
            .y_grid_spacer(move |_input| plot_kit::grid_marks(0.0, yhi, ystep))
            .label_formatter(move |name, p| {
                plot_kit::data_tooltip(
                    name,
                    &[
                        ("Vd", format!("{} V", fmt_num3(sign * p.x))),
                        ("|Id|", format!("{}A", eng_tick(p.y))),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([0.0, 0.0], [xhi, yhi]));
                for (rank, &i) in order.iter().enumerate() {
                    let s = &series[i];
                    let hue = super::gate_ramp(rank as f64 / (n - 1).max(1) as f64);
                    let gate = format!("V_G {} V", fmt_num3(s.vg));
                    draw_series(plot_ui, s, hue, &gate, measured_mode);
                }
            });
        if fit_failed {
            ui_kit::status_badge_line(
                ui,
                if has_measured { "measured" } else { "warn" },
                BadgeTone::Warning,
                StatusLineText::Wrapped(OUTPUT_FIT_FAILED_MESSAGE),
                |_| {},
            );
        } else if has_device && series.is_empty() {
            let message = if measured_mode {
                "Output data loaded, but no output fit series was found."
            } else {
                "No output fit series."
            };
            ui_kit::status_badge_line(
                ui,
                if measured_mode { "warn" } else { "empty" },
                BadgeTone::Warning,
                StatusLineText::Wrapped(message),
                |_| {},
            );
        }

        if has_model && measured_mode {
            let key = super::gate_ramp(0.65);
            super::measured_model_legend(
                ui,
                key,
                key,
                LegendMark::SolidLine,
                LegendMark::DashedLine,
            );
        } else if has_model {
            super::model_legend(ui, super::gate_ramp(0.65), LegendMark::SolidLine);
        } else {
            super::empty_legend(ui);
        }
    });
}

/// Draw one gate sub-sweep. Measured mode: model dashed under the measured solid
/// line. Predicted mode: the model curve IS the line, drawn solid.
fn draw_series(
    plot_ui: &mut egui_plot::PlotUi,
    s: &OutputSeries,
    hue: egui::Color32,
    gate: &str,
    measured_mode: bool,
) {
    if measured_mode {
        if !s.modelled.is_empty() {
            plot_ui.line(
                Line::new("model", PlotPoints::from(s.modelled.clone()))
                    .color(hue)
                    .width(1.2_f32)
                    .style(LineStyle::dashed_loose()),
            );
        }
        plot_ui.line(
            Line::new(gate.to_string(), PlotPoints::from(s.measured.clone()))
                .color(hue)
                .width(1.6_f32),
        );
    } else {
        plot_ui.line(
            Line::new(gate.to_string(), PlotPoints::from(s.modelled.clone()))
                .color(hue)
                .width(1.6_f32),
        );
    }
}
