//! GM plot: the active model's `g_m = dId/dVg` (S) vs `V_G` on
//! LINEAR axes, the way a transconductance trace is conventionally read. The model
//! `g_m` (the smooth derivative of the model transfer over a clean `V_G` linspace)
//! is the solid line; the measured `g_m` (the noisier `np.gradient` of the measured
//! `Id`) scatters under it as a fit check. Follows the shared instrument-plot voice
//! (`plot_kit`) like `plot.rs`/`output_plot.rs`.

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, PlotBounds, PlotPoints, Points};

use crate::format_ui::{eng_tick, fmt_num3};
use crate::plot_kit::{self, LegendMark};
use crate::theme::SUISEI_MAIN;
use crate::ui_kit;
use crate::workspaces::modelfit::state::ModelFitState;

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "TRANSCONDUCTANCE", None);
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let (modelled, measured, vlo, vhi, sign) = if let Some(entry) = selected {
            let dev = entry.device();
            let model = dev.model(state.selected_fit_model());
            let (vlo, vhi) = dev.vg_span();
            (
                model.gm_series(),
                dev.measured_gm_series(),
                vlo,
                vhi,
                dev.polarity().sign(),
            )
        } else {
            (Vec::new(), Vec::new(), 0.0, 1.0, 1.0)
        };

        // Linear y bounds from both series, anchored at 0 (g_m ≥ 0 in the on-region).
        let mut ymax = 0.0_f64;
        for p in modelled.iter().chain(&measured) {
            if p[1].is_finite() {
                ymax = ymax.max(p[1]);
            }
        }
        if !ymax.is_finite() || ymax <= 0.0 {
            ymax = 1.0;
        }
        let yhi = ymax * 1.06;
        let hue = SUISEI_MAIN;

        let base = egui::TextStyle::Body.resolve(ui.style());
        let xstep = plot_kit::nice_axis_step(vhi - vlo);
        let ystep = plot_kit::nice_axis_step(yhi);
        let x_hints = plot_kit::muted_axis(Axis::X, "V<sub>G</sub> (V)", base.clone())
            .formatter(move |mark: GridMark, _range| {
                plot_kit::numeric_tick_label(has_device, mark.value)
            })
            .label_spacing(egui::Rangef::new(24.0, 40.0));
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(Axis::Y, "g<sub>m</sub> (S)", base)
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::engineering_tick_label(has_device, mark.value)
                })
                .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );
        let (xlo, xhi) = super::gate_voltage_plot_bounds(vlo, vhi);

        super::plot_grid(ui, "modelfit_gm_plot")
            .invert_x(sign < 0.0)
            .custom_x_axes(vec![x_hints])
            .custom_y_axes(vec![y_hints])
            .x_grid_spacer(move |_input| plot_kit::grid_marks(vlo, vhi, xstep))
            .y_grid_spacer(move |_input| plot_kit::grid_marks(0.0, yhi, ystep))
            .label_formatter(|_name, p| {
                plot_kit::data_tooltip(
                    "",
                    &[
                        ("Vg", format!("{} V", fmt_num3(p.x))),
                        ("gm", format!("{}S", eng_tick(p.y))),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([xlo, 0.0], [xhi, yhi]));
                // Measured scatter under the model line: the np.gradient of measured Id
                // is intrinsically noisy on the linear axis, so the points stay small
                // enough to read as a fit check without obscuring the model line.
                if !measured.is_empty() {
                    plot_ui.points(
                        Points::new("measured", PlotPoints::from(measured))
                            .radius(1.3_f32)
                            .color(hue),
                    );
                }
                if !modelled.is_empty() {
                    plot_ui.line(
                        Line::new("g<sub>m</sub>", PlotPoints::from(modelled))
                            .color(hue)
                            .width(1.6_f32),
                    );
                }
            });

        if has_device {
            super::measured_model_legend(ui, hue, hue, LegendMark::Dot, LegendMark::SolidLine);
        } else {
            super::empty_legend(ui);
        }
    });
}
