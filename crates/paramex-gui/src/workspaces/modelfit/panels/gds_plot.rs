//! GDS plot: `g_ds = dId/dVd` (S) vs `V_D`, one curve per gate sub-sweep,
//! on LINEAR axes. Loaded output data shows measured derivative dots with model
//! lines; transfer-only devices show the predicted model family. Gate voltage uses
//! the same Studio-Stellar blue ramp as `output_plot.rs`.

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, PlotBounds, PlotPoints, Points};
use paramex_core::modelfit::OutputSeries;

use crate::format_ui::{eng_tick, fmt_num3, OUTPUT_FIT_FAILED_MESSAGE};
use crate::plot_kit::{self, LegendMark};
use crate::ui_kit::{self, BadgeTone, StatusLineText};
use crate::workspaces::modelfit::state::ModelFitState;

fn gds_plot_bounds(family: &[OutputSeries]) -> (f64, f64, f64) {
    let (mut xmax, mut ymin, mut ymax) = (0.0_f64, 0.0_f64, 0.0_f64);
    for series in family {
        for point in series.measured.iter().chain(&series.modelled) {
            if point[0].is_finite() && point[1].is_finite() {
                xmax = xmax.max(point[0]);
                ymin = ymin.min(point[1]);
                ymax = ymax.max(point[1]);
            }
        }
    }
    let xhi = if xmax > 0.0 { xmax * 1.04 } else { 1.04 };
    let magnitude = ymin.abs().max(ymax.abs());
    if magnitude == 0.0 {
        return (xhi, 0.0, 1.06);
    }
    let padding = magnitude * 0.06;
    let ylo = if ymin < 0.0 { ymin - padding } else { 0.0 };
    let yhi = if ymax > 0.0 { ymax + padding } else { 0.0 };
    (xhi, ylo, yhi)
}

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let (family, sign, has_output_curves) = if let Some(entry) = selected {
            let device = entry.device();
            (
                device.model(state.selected_fit_model()).gds_series(),
                device.polarity().sign(),
                device.has_output_curves(),
            )
        } else {
            (Vec::new(), 1.0, false)
        };
        let has_measured = family.iter().any(|s| s.measured.len() >= 2);
        let has_model = family.iter().any(|s| s.modelled.len() >= 2);
        let fit_failed = has_output_curves && !has_model;

        // Keep zero visible while expanding for signed measured derivatives.
        let (xhi, ylo, yhi) = gds_plot_bounds(&family);

        // Order the curves by on-direction drive so the color ramp tracks increasing
        // V_G regardless of file row order (the gates carry the device-frame sign).
        let order = super::gate_drive_order(&family, sign);
        let gate_metadata = super::gate_drive_metadata(&family, &order);
        let n = order.len().max(1);

        ui_kit::section_header(ui, "OUTPUT CONDUCTANCE", gate_metadata.as_deref());

        let base = egui::TextStyle::Body.resolve(ui.style());
        let xstep = plot_kit::nice_axis_step(xhi);
        let ystep = plot_kit::nice_axis_step(yhi - ylo);
        let x_hints = plot_kit::muted_axis(Axis::X, "V<sub>D</sub> (V)", base.clone())
            .formatter(move |mark: GridMark, _range| {
                plot_kit::numeric_tick_label(has_device, sign * mark.value)
            })
            .label_spacing(egui::Rangef::new(24.0, 40.0));
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(Axis::Y, "g<sub>ds</sub> (S)", base)
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::engineering_tick_label(has_device, mark.value)
                })
                .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );

        // A failed fit keeps both its status row and the measured-only legend.
        // Reserve a second footer so that pair cannot grow this fixed tile into
        // the next graph row.
        let extra_footer = if fit_failed && has_measured {
            plot_kit::PLOT_FOOTER_RESERVE
        } else {
            0.0
        };
        let plot_height = plot_kit::plot_body_height(ui.available_height() - extra_footer);
        super::plot_grid(ui, "modelfit_gds_plot")
            .height(plot_height)
            .custom_x_axes(vec![x_hints])
            .custom_y_axes(vec![y_hints])
            .x_grid_spacer(move |_input| plot_kit::grid_marks(0.0, xhi, xstep))
            .y_grid_spacer(move |_input| plot_kit::grid_marks(ylo, yhi, ystep))
            .label_formatter(move |name, p| {
                plot_kit::data_tooltip(
                    name,
                    &[
                        ("Vd", format!("{} V", fmt_num3(sign * p.x))),
                        ("gds", format!("{}S", eng_tick(p.y))),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([0.0, ylo], [xhi, yhi]));
                for (rank, &i) in order.iter().enumerate() {
                    let s = &family[i];
                    let hue = super::gate_ramp(rank as f64 / (n - 1).max(1) as f64);
                    let gate = format!("V_G {} V", fmt_num3(s.vg));
                    if s.measured.len() >= 2 {
                        plot_ui.points(
                            Points::new(gate.clone(), PlotPoints::from(s.measured.clone()))
                                .radius(1.3_f32)
                                .color(hue),
                        );
                    }
                    if s.modelled.len() >= 2 {
                        plot_ui.line(
                            Line::new(gate, PlotPoints::from(s.modelled.clone()))
                                .color(hue)
                                .width(1.6_f32),
                        );
                    }
                }
            });
        if fit_failed {
            ui_kit::status_badge_line(
                ui,
                "warn",
                BadgeTone::Warning,
                StatusLineText::Wrapped(OUTPUT_FIT_FAILED_MESSAGE),
                |_| {},
            );
        }
        let key = super::gate_ramp(0.65);
        if has_measured && has_model {
            super::measured_model_legend(ui, key, key, LegendMark::Dot, LegendMark::SolidLine);
        } else if has_measured {
            plot_kit::centered_legend_row(
                ui,
                &[(key, LegendMark::Dot, super::MEASURED_LEGEND_LABEL)],
            );
        } else if has_model {
            super::model_legend(ui, key, LegendMark::SolidLine);
        } else {
            super::empty_legend(ui);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::gds_plot_bounds;
    use paramex_core::modelfit::OutputSeries;

    #[test]
    fn bounds_keep_negative_measured_conductance_visible() {
        let family = [OutputSeries {
            vg: 1.0,
            measured: vec![[0.0, -2.0e-6], [1.0, -1.0e-6]],
            modelled: Vec::new(),
        }];

        let (_, ylo, yhi) = gds_plot_bounds(&family);
        assert!(ylo < -2.0e-6);
        assert!(yhi >= 0.0);
    }
}
