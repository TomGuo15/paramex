//! Segmented-control recipes for workspace toggles and graph mode tabs.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Margin, Stroke, StrokeKind, WidgetInfo, WidgetType,
};

use crate::richtext;
use crate::theme::{tokens, utility_white_alpha, INK_RAISED};

use super::bold_family;
use super::buttons::{
    add_state_button, button_label_job, filled_states, outlined_states, variant_states_in, Variant,
    BUTTON_HEIGHT, SEMANTIC_BUTTON_HOVER_ALPHA, SEMANTIC_BUTTON_PRESS_ALPHA,
};

pub const HEADER_TAB_HEIGHT: f32 = 20.0;
const HEADER_TAB_UNDERLINE_HEIGHT: f32 = 2.0;
const HEADER_TAB_UNDERLINE_INSET: f32 = 6.0;

/// Where a segmented control sits: inside a white card, or on the ink banner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SegStyle {
    Card,
    Banner,
}

/// Hover fill for an INACTIVE segment. Card: a soft primary alpha
/// wash; Banner: a light utility wash over the ink track (rest is transparent).
fn segment_hover_fill(style: SegStyle) -> Color32 {
    match style {
        SegStyle::Card => tokens().accent_soft,
        SegStyle::Banner => utility_white_alpha(26),
    }
}

/// Pure: `(fill, text color)` for one segment given its style and whether it is active.
pub fn segment_colors(style: SegStyle, active: bool) -> (Color32, Color32) {
    let t = tokens();
    match (style, active) {
        (SegStyle::Card, true) => (Color32::TRANSPARENT, t.primary),
        (SegStyle::Card, false) => (Color32::TRANSPARENT, t.ink_soft),
        (SegStyle::Banner, true) => (t.surface, t.ink),
        (SegStyle::Banner, false) => (Color32::TRANSPARENT, t.surface),
    }
}

/// An N-segment control. Card tabs are flat, intrinsic-width title-rail actions;
/// banner segments retain their rounded track. `Some(w)` fixes segment width.
/// Labels may carry `<sub>`/`<sup>` markup. Returns the clicked index.
pub fn segmented(
    ui: &mut egui::Ui,
    labels: &[&str],
    active: usize,
    style: SegStyle,
    seg_w: Option<f32>,
) -> Option<usize> {
    segmented_impl(ui, labels, None, active, style, seg_w)
}

pub fn segmented_with_accessibility_labels(
    ui: &mut egui::Ui,
    labels: &[&str],
    accessibility_labels: &[&str],
    active: usize,
    style: SegStyle,
    seg_w: Option<f32>,
) -> Option<usize> {
    segmented_impl(ui, labels, Some(accessibility_labels), active, style, seg_w)
}

fn segmented_impl(
    ui: &mut egui::Ui,
    labels: &[&str],
    accessibility_labels: Option<&[&str]>,
    active: usize,
    style: SegStyle,
    seg_w: Option<f32>,
) -> Option<usize> {
    if style == SegStyle::Card {
        return card_header_tabs(ui, labels, accessibility_labels, active, seg_w);
    }

    let t = tokens();
    let mut clicked = None;
    egui::Frame::new()
        .fill(INK_RAISED)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(7))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let n = labels.len().max(1) as f32;
                let w = seg_w
                    .unwrap_or_else(|| ((ui.available_width() - 2.0 * (n - 1.0)) / n).max(40.0));
                for (idx, label) in labels.iter().enumerate() {
                    let is_active = idx == active;
                    let (mut fill, mut text) = segment_colors(style, is_active);
                    if !ui.is_enabled() {
                        fill = t.surface;
                        text = t.ink;
                    }
                    // Every segment carries its explicit fill (Card-inactive = the page
                    // tint so it reads as a switchable page). An explicit fill pins every
                    // widget state, so the hover wash is hand-rolled off the PREVIOUS
                    // frame's response (the file-row idiom) — inactive segments only;
                    // the active segment is the current page and inert to hover.
                    let id = ui.next_auto_id();
                    let hovered =
                        !is_active && ui.ctx().read_response(id).is_some_and(|r| r.hovered());
                    if hovered {
                        fill = segment_hover_fill(style);
                    }
                    let job =
                        richtext::layout_sub_sup(label, FontId::new(12.5, bold_family(ui)), text);
                    let btn = egui::Button::new(job)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(5))
                        .min_size(egui::vec2(w, 26.0))
                        .fill(fill);
                    let resp = ui.add(btn);
                    if let Some(label) = accessibility_labels.and_then(|labels| labels.get(idx)) {
                        resp.widget_info(|| {
                            WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label)
                        });
                    }
                    if resp.clicked() {
                        clicked = Some(idx);
                    }
                }
            });
        });
    clicked
}

fn card_header_tabs(
    ui: &mut egui::Ui,
    labels: &[&str],
    accessibility_labels: Option<&[&str]>,
    active: usize,
    seg_w: Option<f32>,
) -> Option<usize> {
    let t = tokens();
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (idx, label) in labels.iter().enumerate() {
            let is_active = idx == active;
            let (_, mut text) = segment_colors(SegStyle::Card, is_active);
            if !ui.is_enabled() {
                text = t.ink;
            }
            let id = ui.next_auto_id();
            let hovered = !is_active && ui.ctx().read_response(id).is_some_and(|r| r.hovered());
            let fill = if hovered {
                segment_hover_fill(SegStyle::Card)
            } else {
                Color32::TRANSPARENT
            };
            let job = richtext::layout_sub_sup(label, FontId::new(12.0, bold_family(ui)), text);
            let button = egui::Button::new(job)
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(3))
                .min_size(egui::vec2(0.0, HEADER_TAB_HEIGHT))
                .fill(fill);
            let resp = match seg_w {
                Some(width) => ui.add_sized(egui::vec2(width, HEADER_TAB_HEIGHT), button),
                None => ui.add(button),
            };
            if let Some(label) = accessibility_labels.and_then(|labels| labels.get(idx)) {
                resp.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label)
                });
            }
            if is_active {
                let underline = egui::Rect::from_min_max(
                    egui::pos2(
                        resp.rect.left() + HEADER_TAB_UNDERLINE_INSET,
                        resp.rect.bottom() - HEADER_TAB_UNDERLINE_HEIGHT,
                    ),
                    egui::pos2(
                        resp.rect.right() - HEADER_TAB_UNDERLINE_INSET,
                        resp.rect.bottom(),
                    ),
                );
                ui.painter()
                    .rect_filled(underline, CornerRadius::same(1), t.primary);
            }
            if resp.has_focus() {
                ui.painter().rect_stroke(
                    resp.rect.shrink(1.0),
                    CornerRadius::same(3),
                    Stroke::new(1.0_f32, t.primary),
                    StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                clicked = Some(idx);
            }
        }
    });
    clicked
}

/// Like [`segmented`] but each segment carries its OWN brand colour (e.g. the forward/backward
/// LINE colours) so the toggle doubles as the graph legend (the separate legend row is
/// dropped). The active segment is FILLED with its colour (white label); the inactive one
/// is OUTLINED in it. Returns the clicked index.
pub fn segmented_two_colored(
    ui: &mut egui::Ui,
    labels: [&str; 2],
    active: usize,
    colors: [Color32; 2],
) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        // Split the full row width between the two segments so the toggle fills the card.
        let seg_w = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(40.0);
        for (idx, label) in labels.iter().enumerate() {
            let color = colors[idx];
            let (states, text) = if !ui.is_enabled() {
                (variant_states_in(ui, Variant::Secondary), tokens().ink)
            } else if idx == active {
                (filled_states(color), tokens().surface)
            } else {
                (
                    outlined_states(
                        color,
                        color,
                        SEMANTIC_BUTTON_HOVER_ALPHA,
                        SEMANTIC_BUTTON_PRESS_ALPHA,
                    ),
                    color,
                )
            };
            let content = button_label_job(ui, label, text);
            let resp = add_state_button(
                ui,
                content,
                states,
                Some(egui::vec2(seg_w, BUTTON_HEIGHT)),
                egui::vec2(0.0, BUTTON_HEIGHT),
            );
            if resp.clicked() {
                clicked = Some(idx);
            }
        }
    });
    clicked
}
