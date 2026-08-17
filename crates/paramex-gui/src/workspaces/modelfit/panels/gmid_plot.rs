//! gm/Id SIZING CURVE: the canonical Silveira–Flandre–Jespers / Murmann design chart —
//! drain-current density `I_D/W` (A/µm, LOG axis) vs transconductance efficiency
//! `g_m/I_D` (V⁻¹) on the x-axis. A designer enters with a target `g_m/I_D` (which fixes
//! the inversion level) and reads off the current density to size the device. The model
//! curve is the solid line; the measured data scatters under it as a fit check. Follows
//! the decade-axis convention of `plot.rs` and the shared instrument-plot voice.

use eframe::egui;
use egui_plot::{Axis, GridMark, Line, PlotBounds, PlotPoints, Points};

use crate::format_ui::{eng_tick, fmt_num3};
use crate::plot_kit::{self, LegendMark};
use crate::theme::SUISEI_MAIN;
use crate::ui_kit;
use crate::workspaces::modelfit::state::ModelFitState;

/// Smallest `I_D/W` allowed onto the log axis so a zero/negative value can't map to -inf.
const LOG_FLOOR: f64 = 1.0e-15;

pub fn show(ui: &mut egui::Ui, state: &ModelFitState) {
    ui_kit::card_slot(ui, |ui| {
        ui_kit::section_header(ui, "GM/ID SIZING", None);
        let selected = state.selected_entry();
        let has_device = selected.is_some();
        let (modelled, measured) = if let Some(entry) = selected {
            let device = entry.device();
            (
                device
                    .model(state.selected_fit_model())
                    .gm_id_sizing_series(),
                device.measured_gm_id_sizing_series(),
            )
        } else {
            (Vec::new(), Vec::new())
        };

        // X bounds from the gm/Id spread (V⁻¹); Y (Id/W) decade bounds from positive values.
        let (mut xlo, mut xhi) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut dmin, mut dmax) = (f64::INFINITY, 0.0_f64);
        for p in modelled.iter().chain(&measured) {
            if p[0].is_finite() {
                xlo = xlo.min(p[0]);
                xhi = xhi.max(p[0]);
            }
            if p[1] > 0.0 && p[1].is_finite() {
                dmin = dmin.min(p[1]);
                dmax = dmax.max(p[1]);
            }
        }
        if !xlo.is_finite() || !xhi.is_finite() || xhi <= xlo {
            xlo = 0.0;
            xhi = 1.0;
        }
        let xpad = (xhi - xlo) * super::GM_ID_X_PAD_FRACTION + 1.0e-6;
        let (xlo, xhi) = (xlo - xpad, xhi + xpad);
        if !dmin.is_finite() || dmax <= 0.0 {
            dmin = 1.0e-9;
            dmax = 1.0e-3;
        }
        let ylo = dmin.log10().floor();
        let yhi = dmax.log10().ceil().max(ylo + 1.0);
        let hue = SUISEI_MAIN;

        let to_log = |pts: &[[f64; 2]]| -> Vec<[f64; 2]> {
            pts.iter()
                .map(|&[g, d]| [g, d.max(LOG_FLOOR).log10()])
                .collect()
        };
        let modelled_log = to_log(&modelled);
        let measured_log = to_log(&measured);

        let base = egui::TextStyle::Body.resolve(ui.style());
        let xstep = plot_kit::nice_axis_step(xhi - xlo);
        let x_hints =
            plot_kit::muted_axis(Axis::X, "g<sub>m</sub>/I<sub>D</sub> (1/V)", base.clone())
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::numeric_tick_label(has_device, mark.value)
                })
                .label_spacing(egui::Rangef::new(24.0, 64.0));
        let step = plot_kit::decade_label_step(ylo as i64, yhi as i64);
        let y_hints = plot_kit::visible_y_axis_gutter(
            plot_kit::muted_axis(Axis::Y, "|I<sub>D</sub>|/W (A/\u{00b5}m)", base)
                .formatter(move |mark: GridMark, _range| {
                    plot_kit::log_decade_tick_label(has_device, mark.value, step)
                })
                .label_spacing(egui::Rangef::new(10.0, 16.0)),
            has_device,
            super::Y_AXIS_MIN_THICKNESS,
        );

        let (glo, ghi) = (ylo as i64, yhi as i64);

        super::plot_grid(ui, "modelfit_gmid_plot")
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
            .x_grid_spacer(move |_input| plot_kit::grid_marks(xlo, xhi, xstep))
            .label_formatter(|_name, p| {
                plot_kit::data_tooltip(
                    "",
                    &[
                        ("gm/Id", format!("{} 1/V", fmt_num3(p.x))),
                        (
                            "|Id|/W",
                            format!("{}A/\u{00b5}m", eng_tick(10f64.powf(p.y))),
                        ),
                    ],
                )
            })
            .show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max([xlo, ylo], [xhi, yhi]));
                if !measured_log.is_empty() {
                    plot_ui.points(
                        Points::new("measured", PlotPoints::from(measured_log))
                            .radius(1.3_f32)
                            .color(hue),
                    );
                }
                if !modelled_log.is_empty() {
                    plot_ui.line(
                        Line::new("I<sub>D</sub>/W", PlotPoints::from(modelled_log))
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
