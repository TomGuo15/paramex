//! FIT plot: the selected device's measured transfer points with the
//! active model curve from the extracted parameters, on a LOG Id axis — so the
//! off-state (IOFF floor -> exponential subthreshold -> on-region) is actually
//! visible, the way a transfer curve is conventionally read. Follows the shared
//! instrument-plot voice (`plot_kit`) and the selector's decade-axis convention
//! (`decade_label_step`): a grid line per decade, labels on a sparse subset.
//! Data = solid points, model = dashed line.

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, LineStyle, PlotBounds, PlotPoints, Points};

use crate::plot_kit::{self, LegendMark};
use crate::theme::SUISEI_MAIN;
use crate::ui_kit;
use crate::workspaces::modelfit::state::ModelFitState;

/// Smallest current allowed onto the log axis, so a zero/negative measured point
/// can't map to -inf. Far below any real off floor.
const LOG_FLOOR: f64 = 1.0e-18;

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "TRANSFER FIT", None);
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let (measured_lin, modelled_lin, vlo, vhi, sign) = if let Some(entry) = selected {
            let dev = entry.device();
            let model = dev.model(state.selected_fit_model());
            (
                dev.measured_points(),
                model.transfer_overlay(),
                dev.vg_span().0,
                dev.vg_span().1,
                dev.polarity().sign(),
            )
        } else {
            (Vec::new(), Vec::new(), 0.0, 1.0, 1.0)
        };

        // Decade bounds from the positive currents across both series.
        let (mut idmin, mut idmax) = (f64::INFINITY, 0.0_f64);
        for p in measured_lin.iter().chain(&modelled_lin) {
            if p[1] > 0.0 {
                idmin = idmin.min(p[1]);
                idmax = idmax.max(p[1]);
            }
        }
        if !idmin.is_finite() || idmax <= 0.0 {
            idmin = 1.0e-12;
            idmax = 1.0e-6;
        }
        let ylo = idmin.log10().floor();
        let yhi = idmax.log10().ceil().max(ylo + 1.0);

        let to_log = |pts: Vec<[f64; 2]>| -> Vec<[f64; 2]> {
            pts.into_iter()
                .map(|[v, i]| [v, i.max(LOG_FLOOR).log10()])
                .collect()
        };
        let measured = to_log(measured_lin);
        let modelled = to_log(modelled_lin);
        let hue = SUISEI_MAIN;

        let base = egui::TextStyle::Body.resolve(ui.style());
        // V_G ticks at a nice step, labelled with the plain value (the default
        // spacer collapsed to a single "0" on a -10..5 p-channel sweep).
        let xstep = plot_kit::nice_axis_step(vhi - vlo);
        let x_hints = plot_kit::muted_axis(Axis::X, "V<sub>G</sub> (V)", base.clone())
            .formatter(move |mark: GridMark, _range| {
                plot_kit::numeric_tick_label(has_device, mark.value)
            })
            .label_spacing(egui::Rangef::new(24.0, 40.0));
        let step = plot_kit::decade_label_step(ylo as i64, yhi as i64);
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(Axis::Y, "|I<sub>D</sub>| (A)", base)
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::log_decade_tick_label(has_device, mark.value, step)
                })
                .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );

        // A decade grid line at each integer power; labels thinned by `step`.
        let (glo, ghi) = (ylo as i64, yhi as i64);
        let (xlo, xhi) = super::gate_voltage_plot_bounds(vlo, vhi);

        super::plot_grid(ui, "modelfit_plot")
            .invert_x(sign < 0.0)
            .custom_x_axes(vec![x_hints])
            .custom_y_axes(vec![y_hints])
            .y_grid_spacer(move |_input| {
                (glo..=ghi)
                    .map(|n| GridMark {
                        value: n as f64,
                        step_size: step as f64,
                    })
                    .collect()
            })
            .x_grid_spacer(move |_input| {
                let lo = (vlo / xstep).ceil() as i64;
                let hi = (vhi / xstep).floor() as i64;
                (lo..=hi)
                    .map(|k| GridMark {
                        value: k as f64 * xstep,
                        step_size: xstep,
                    })
                    .collect()
            })
            .label_formatter(|_name, p| {
                plot_kit::data_tooltip(
                    "",
                    &[
                        ("Vg", format!("{} V", crate::format_ui::fmt_num3(p.x))),
                        (
                            "|Id|",
                            format!("{}A", crate::format_ui::eng_tick(10f64.powf(p.y))),
                        ),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([xlo, ylo], [xhi, yhi]));
                // Model below (dashed), measured points on top (solid).
                plot_ui.line(
                    Line::new("model", PlotPoints::from(modelled))
                        .color(hue)
                        .width(1.5_f32)
                        .style(LineStyle::dashed_loose()),
                );
                plot_ui.points(
                    Points::new("measured", PlotPoints::from(measured))
                        .radius(2.5_f32)
                        .color(hue),
                );
            });

        if has_device {
            super::measured_model_legend(ui, hue, hue, LegendMark::Dot, LegendMark::DashedLine);
        } else {
            super::empty_legend(ui);
        }
    });
}
