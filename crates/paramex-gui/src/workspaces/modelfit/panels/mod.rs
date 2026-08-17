//! Model Fit workspace panels.

use eframe::egui;
use egui_plot::Plot;
use std::hash::Hash;

use crate::format_ui::fmt_num3;
use crate::plot_kit;
use crate::theme::{SUISEI_LIGHT, SUISEI_MAIN};
use paramex_core::modelfit::OutputSeries;

pub mod devices;
pub mod gain_plot;
pub mod gds_plot;
pub mod gm_plot;
pub mod gmid_plot;
pub mod inputs;
pub mod output_plot;
pub mod plot;
pub mod summary;

const Y_AXIS_MIN_THICKNESS: f32 = 58.0;
const GATE_VOLTAGE_X_PAD_FRACTION: f64 = 0.04;
const GM_ID_X_PAD_FRACTION: f64 = 0.30;
const MEASURED_LEGEND_LABEL: &str = "measured";
const MODEL_LEGEND_LABEL: &str = "model";

pub(super) fn plot_grid<'a>(ui: &egui::Ui, id: impl Hash) -> Plot<'a> {
    plot_kit::quiet_grid(
        Plot::new(id)
            .height(plot_kit::plot_body_height(ui.available_height()))
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false),
    )
}

pub(super) fn gate_voltage_plot_bounds(vlo: f64, vhi: f64) -> (f64, f64) {
    let pad = (vhi - vlo) * GATE_VOLTAGE_X_PAD_FRACTION;
    (vlo - pad, vhi + pad)
}

pub(super) fn gate_drive_order(family: &[OutputSeries], sign: f64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..family.len()).collect();
    order.sort_by(|&a, &b| {
        (sign * family[a].vg)
            .partial_cmp(&(sign * family[b].vg))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order
}

pub(super) fn gate_drive_metadata(family: &[OutputSeries], order: &[usize]) -> Option<String> {
    let first = family.get(*order.first()?)?.vg;
    let last = family.get(*order.last()?)?.vg;
    Some(if first == last {
        format!("V<sub>G</sub> {} V", fmt_num3(first))
    } else {
        format!(
            "V<sub>G</sub> {} \u{2192} {} V",
            fmt_num3(first),
            fmt_num3(last)
        )
    })
}

pub(super) fn empty_legend(ui: &mut egui::Ui) {
    plot_kit::centered_legend_row(ui, &[]);
}

pub(super) fn model_legend(ui: &mut egui::Ui, hue: egui::Color32, mark: plot_kit::LegendMark) {
    plot_kit::centered_legend_row(ui, &[(hue, mark, MODEL_LEGEND_LABEL)]);
}

pub(super) fn measured_model_legend(
    ui: &mut egui::Ui,
    hue: egui::Color32,
    measured_hue: egui::Color32,
    measured_mark: plot_kit::LegendMark,
    model_mark: plot_kit::LegendMark,
) {
    plot_kit::centered_legend_row(
        ui,
        &[
            (measured_hue, measured_mark, MEASURED_LEGEND_LABEL),
            (hue, model_mark, MODEL_LEGEND_LABEL),
        ],
    );
}

/// Gate-voltage color ramp within the Studio Stellar group: pale blue (low drive)
/// → vivid blue (high drive). Floored so even the lowest curve stays visible on the
/// white card. `frac` ∈ [0, 1]. Shared by the output-family panels (output, gds).
pub(super) fn gate_ramp(frac: f64) -> egui::Color32 {
    let t = (0.50 + 0.50 * frac.clamp(0.0, 1.0)) as f32;
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    egui::Color32::from_rgb(
        lerp(SUISEI_LIGHT.r(), SUISEI_MAIN.r()),
        lerp(SUISEI_LIGHT.g(), SUISEI_MAIN.g()),
        lerp(SUISEI_LIGHT.b(), SUISEI_MAIN.b()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plot_body_height_reserves_footer_and_keeps_minimum() {
        assert_eq!(plot_kit::plot_body_height(400.0), 374.0);
        assert_eq!(plot_kit::plot_body_height(130.0), plot_kit::MIN_PLOT_BODY_H);
    }

    #[test]
    fn gate_voltage_bounds_keep_endpoint_ticks_inside_the_plot() {
        let (lo, hi) = gate_voltage_plot_bounds(-10.0, 5.0);
        assert!(lo < -10.0 && hi > 5.0);
    }

    #[test]
    fn lowest_gate_curve_keeps_three_to_one_surface_contrast() {
        fn channel(value: u8) -> f64 {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        fn luminance(color: egui::Color32) -> f64 {
            0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
        }

        let low = luminance(gate_ramp(0.0));
        let surface = luminance(crate::theme::tokens().surface);
        let contrast = (surface.max(low) + 0.05) / (surface.min(low) + 0.05);
        assert!(contrast >= 3.0, "lowest gate curve contrast was {contrast}");
    }

    #[test]
    fn gate_drive_metadata_follows_device_frame_color_order() {
        let series = |vg| OutputSeries {
            vg,
            measured: Vec::new(),
            modelled: Vec::new(),
        };
        let n_channel = [series(8.0), series(2.0)];
        let order = gate_drive_order(&n_channel, 1.0);
        assert_eq!(order, [1, 0]);
        assert_eq!(
            gate_drive_metadata(&n_channel, &order).as_deref(),
            Some("V<sub>G</sub> 2 \u{2192} 8 V")
        );

        let p_channel = [series(-8.0), series(-2.0)];
        let order = gate_drive_order(&p_channel, -1.0);
        assert_eq!(order, [1, 0]);
        assert_eq!(
            gate_drive_metadata(&p_channel, &order).as_deref(),
            Some("V<sub>G</sub> -2 \u{2192} -8 V")
        );
    }
}
