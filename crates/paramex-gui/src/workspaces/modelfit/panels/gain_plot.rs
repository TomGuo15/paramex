//! GAIN plot: the active model's single-transistor voltage-gain ceiling
//! `A_v = g_m/g_ds` (dimensionless) vs transconductance efficiency `g_m/I_D` (V⁻¹) — the
//! standard gm/Id design chart (gm/Id on the x-axis), the companion to the sizing curve.
//! One point per output-family gate step (each a saturation operating point), connected
//! in gm/Id order. Follows the shared instrument-plot voice (`plot_kit`).

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, PlotBounds, PlotPoints, Points};

use crate::format_ui::{eng_tick, fmt_num3};
use crate::plot_kit::{self, LegendMark};
use crate::theme::SUISEI_MAIN;
use crate::ui_kit;
use crate::workspaces::modelfit::state::ModelFitState;

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "INTRINSIC GAIN", None);
        // (gm/Id, A_v) per gate step, already sorted by ascending gm/Id.
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let points = if let Some(entry) = selected {
            entry
                .device()
                .model(state.selected_fit_model())
                .intrinsic_gain_series()
        } else {
            Vec::new()
        };

        // X bounds from the gm/Id spread (V⁻¹); Y (A_v) anchored at 0.
        let (mut xlo, mut xhi, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY, 0.0_f64);
        for p in &points {
            if p[0].is_finite() {
                xlo = xlo.min(p[0]);
                xhi = xhi.max(p[0]);
            }
            if p[1].is_finite() {
                ymax = ymax.max(p[1]);
            }
        }
        if !xlo.is_finite() || !xhi.is_finite() || xhi <= xlo {
            xlo = 0.0;
            xhi = 1.0;
        }
        let xpad = (xhi - xlo) * super::GM_ID_X_PAD_FRACTION + 1.0e-6;
        let (xlo, xhi) = (xlo - xpad, xhi + xpad);
        if !ymax.is_finite() || ymax <= 0.0 {
            ymax = 1.0;
        }
        let yhi = ymax * 1.06;
        let hue = SUISEI_MAIN;

        let base = egui::TextStyle::Body.resolve(ui.style());
        let xstep = plot_kit::nice_axis_step(xhi - xlo);
        let ystep = plot_kit::nice_axis_step(yhi);
        let x_hints =
            plot_kit::muted_axis(Axis::X, "g<sub>m</sub>/I<sub>D</sub> (1/V)", base.clone())
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::numeric_tick_label(has_device, mark.value)
                })
                .label_spacing(egui::Rangef::new(24.0, 64.0));
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(
                Axis::Y,
                "A<sub>v</sub> = g<sub>m</sub>/g<sub>ds</sub>",
                base,
            )
            .formatter(move |mark: GridMark, _range| {
                plot_kit::engineering_tick_label(has_device, mark.value)
            })
            .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );

        super::plot_grid(ui, "modelfit_gain_plot")
            .custom_x_axes(vec![x_hints])
            .custom_y_axes(vec![y_hints])
            .x_grid_spacer(move |_input| plot_kit::grid_marks(xlo, xhi, xstep))
            .y_grid_spacer(move |_input| plot_kit::grid_marks(0.0, yhi, ystep))
            .label_formatter(|_name, p| {
                plot_kit::data_tooltip(
                    "",
                    &[
                        ("gm/Id", format!("{} 1/V", fmt_num3(p.x))),
                        ("Av", eng_tick(p.y)),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([xlo, 0.0], [xhi, yhi]));
                plot_ui.line(
                    Line::new("Av", PlotPoints::from(points.clone()))
                        .color(hue)
                        .width(1.6_f32),
                );
                plot_ui.points(
                    Points::new("Av", PlotPoints::from(points.clone()))
                        .radius(2.6_f32)
                        .color(hue),
                );
            });
        if has_device && !points.is_empty() {
            super::model_legend(ui, hue, LegendMark::SolidLine);
        } else {
            super::empty_legend(ui);
        }
    });
}
