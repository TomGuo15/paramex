use eframe::egui::{self, text::LayoutJob, Color32, CornerRadius, FontId, Response, Stroke};

use crate::richtext;
use crate::theme::{token_alpha, tokens};

use super::bold_family;

mod state;

pub(super) use state::{add_state_button, filled_states, outlined_states, variant_states_in};

/// Button variants matching the original's palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Primary,   // blue filled, white text
    Secondary, // white fill, blue text + border
    Danger,    // white fill, red text + border
    Warning,   // white fill, ink text + yellow border
}

pub const BUTTON_HEIGHT: f32 = 30.0;
pub const HEADER_ACTION_HEIGHT: f32 = 20.0;
pub const BUTTON_CORNER_RADIUS: u8 = 7;
pub const BUTTON_FONT_SIZE: f32 = 12.5;
pub const SECONDARY_BUTTON_HOVER_ALPHA: u8 = 15;
pub const SECONDARY_BUTTON_PRESS_ALPHA: u8 = 31;
pub const SEMANTIC_BUTTON_HOVER_ALPHA: u8 = 20;
pub const SEMANTIC_BUTTON_PRESS_ALPHA: u8 = 41;
pub const CLOSE_BUTTON_HOVER_ALPHA: u8 = 26;
pub const CLOSE_BUTTON_PRESS_ALPHA: u8 = 41;

pub(super) fn button_label_job(ui: &egui::Ui, markup: &str, color: Color32) -> LayoutJob {
    richtext::layout_sub_sup(
        markup,
        FontId::new(BUTTON_FONT_SIZE, bold_family(ui)),
        color,
    )
}

fn variant_colors(v: Variant) -> (Color32, Color32, Color32) {
    let t = tokens();
    match v {
        Variant::Primary => (t.primary, t.surface, t.primary),
        Variant::Secondary => (t.surface, t.primary, t.border),
        Variant::Danger => (t.surface, t.red, t.red),
        // Yellow is too quiet as small text on white. Use it as the button
        // surface/stroke and keep Studio dark copy for contrast.
        Variant::Warning => (t.yellow, t.ink, t.yellow),
    }
}

/// `(fill, text, stroke)` for a variant button given the host ui's enabled
/// state. egui paints a disabled Ui at `disabled_alpha` (0.5), which turns the
/// brand fill/outline into a washed-out "half-pressed" button (pale blue,
/// white text) — render disabled buttons white/grey instead, with colors
/// picked to survive the alpha multiply (ink text → mid-grey, `ink_soft`
/// hairline → light grey).
fn variant_colors_in(ui: &egui::Ui, v: Variant) -> (Color32, Color32, Color32) {
    if ui.is_enabled() {
        variant_colors(v)
    } else {
        let t = tokens();
        (t.surface, t.ink, t.ink_soft)
    }
}

/// A full-width variant button (the common case inside cards). The label may
/// carry `<sub>`/`<sup>` markup (e.g. "Estimate C<sub>ox</sub>") — the shared
/// `button_label_job` parses it.
pub fn button_full(ui: &mut egui::Ui, label: &str, v: Variant) -> Response {
    let w = ui.available_width();
    let text = variant_colors_in(ui, v).1;
    let states = variant_states_in(ui, v);
    let content = button_label_job(ui, label, text);
    add_state_button(
        ui,
        content,
        states,
        Some(egui::vec2(w, BUTTON_HEIGHT)),
        egui::vec2(0.0, BUTTON_HEIGHT),
    )
}

/// A content-width variant button (for side-by-side rows).
pub fn button(ui: &mut egui::Ui, label: &str, v: Variant) -> Response {
    let text = variant_colors_in(ui, v).1;
    let states = variant_states_in(ui, v);
    let content = button_label_job(ui, label, text);
    add_state_button(ui, content, states, None, egui::vec2(0.0, BUTTON_HEIGHT))
}

/// Compact borderless action for the shared card-title rail. Primary actions
/// use brand-blue text; secondary actions stay quiet until hover.
pub fn header_action(ui: &mut egui::Ui, label: &str, v: Variant) -> Response {
    let t = tokens();
    let enabled = ui.is_enabled();
    let (text, hue) = if !enabled {
        (t.ink, t.ink_soft)
    } else {
        match v {
            Variant::Primary => (t.primary, t.primary),
            Variant::Secondary => (t.ink_soft, t.primary),
            Variant::Danger => (t.red, t.red),
            Variant::Warning => (t.ink, t.yellow),
        }
    };
    let rest = Color32::TRANSPARENT;
    let hover = if enabled {
        token_alpha(hue, SECONDARY_BUTTON_HOVER_ALPHA)
    } else {
        rest
    };
    let press = if enabled {
        token_alpha(hue, SECONDARY_BUTTON_PRESS_ALPHA)
    } else {
        rest
    };
    let content = richtext::layout_sub_sup(label, FontId::new(10.5, bold_family(ui)), text);

    ui.scope(|ui| {
        fn set(wv: &mut egui::style::WidgetVisuals, fill: Color32) {
            wv.weak_bg_fill = fill;
            wv.bg_fill = fill;
            wv.bg_stroke = Stroke::NONE;
            wv.corner_radius = CornerRadius::same(4);
        }
        let widgets = &mut ui.style_mut().visuals.widgets;
        set(&mut widgets.noninteractive, rest);
        set(&mut widgets.inactive, rest);
        set(&mut widgets.hovered, hover);
        set(&mut widgets.active, press);
        ui.spacing_mut().button_padding = egui::vec2(2.0, 0.0);
        ui.add(egui::Button::new(content).min_size(egui::vec2(0.0, HEADER_ACTION_HEIGHT)))
    })
    .inner
}

/// A borderless close/remove affordance: a hand-painted crisp cross, quiet gray
/// at rest, washing red with a pointer cursor on hover (the boxed text-glyph "×"
/// button read as amateur — user 2026-06-12). `label` is the accesskit name the
/// kittest guards target ("Dismiss", "Remove layer", …). Honors a disabled
/// enclosing `add_enabled_ui` scope: no click sense, muted cross, no wash.
pub fn close_button(ui: &mut egui::Ui, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), sense);
    let t = tokens();
    let (wash, cross) = if !enabled {
        (Color32::TRANSPARENT, t.ink_soft)
    } else if resp.is_pointer_button_down_on() {
        (token_alpha(t.red, CLOSE_BUTTON_PRESS_ALPHA), t.red)
    } else if resp.hovered() {
        (token_alpha(t.red, CLOSE_BUTTON_HOVER_ALPHA), t.red)
    } else {
        (Color32::TRANSPARENT, t.ink_soft)
    };
    if wash != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(5), wash);
    }
    let c = rect.center();
    let r = 3.5;
    let s = egui::Stroke::new(1.5_f32, cross);
    ui.painter()
        .line_segment([c + egui::vec2(-r, -r), c + egui::vec2(r, r)], s);
    ui.painter()
        .line_segment([c + egui::vec2(-r, r), c + egui::vec2(r, -r)], s);
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let owned = label.to_owned();
    resp.widget_info(move || {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, owned.clone())
    });
    resp
}

#[derive(Clone, Copy)]
pub enum OutputActionIcon {
    Attach,
    Detach,
}

pub fn output_action_icon_button(
    ui: &mut egui::Ui,
    label: &str,
    icon: OutputActionIcon,
) -> Response {
    let enabled = ui.is_enabled();
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), sense);
    let t = tokens();
    let (wash, stroke_color) = if !enabled {
        (Color32::TRANSPARENT, t.ink_soft)
    } else if resp.is_pointer_button_down_on() {
        (token_alpha(t.primary, 41), t.primary)
    } else if resp.hovered() {
        (token_alpha(t.primary, 26), t.primary)
    } else {
        (Color32::TRANSPARENT, t.ink_soft)
    };
    if wash != Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(5), wash);
    }
    let c = rect.center();
    let s = egui::Stroke::new(1.5_f32, stroke_color);
    let dir = match icon {
        OutputActionIcon::Attach => -1.0,
        OutputActionIcon::Detach => 1.0,
    };
    ui.painter().line_segment(
        [
            c + egui::vec2(0.0, -6.0 * dir),
            c + egui::vec2(0.0, 2.0 * dir),
        ],
        s,
    );
    ui.painter().line_segment(
        [
            c + egui::vec2(-3.5, -1.5 * dir),
            c + egui::vec2(0.0, 2.0 * dir),
        ],
        s,
    );
    ui.painter().line_segment(
        [
            c + egui::vec2(3.5, -1.5 * dir),
            c + egui::vec2(0.0, 2.0 * dir),
        ],
        s,
    );
    ui.painter().line_segment(
        [
            c + egui::vec2(-5.0, 6.0 * dir),
            c + egui::vec2(5.0, 6.0 * dir),
        ],
        s,
    );
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let owned = label.to_owned();
    resp.widget_info(move || {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, owned.clone())
    });
    resp
}

/// A content-width button filled with an explicit brand colour + white label (used for
/// the single-sweep "Forward" marker so it matches the forward line colour).
pub fn colored_button(ui: &mut egui::Ui, label: &str, color: Color32) -> Response {
    let w = ui.available_width();
    let (states, text) = if ui.is_enabled() {
        (filled_states(color), tokens().surface)
    } else {
        (variant_states_in(ui, Variant::Secondary), tokens().ink)
    };
    let content = button_label_job(ui, label, text);
    add_state_button(
        ui,
        content,
        states,
        Some(egui::vec2(w, BUTTON_HEIGHT)),
        egui::vec2(0.0, BUTTON_HEIGHT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_buttons_match_their_text_and_outline_colors() {
        let t = tokens();
        assert_eq!(variant_colors(Variant::Danger), (t.surface, t.red, t.red));
        assert_eq!(
            variant_colors(Variant::Warning),
            (t.yellow, t.ink, t.yellow)
        );
    }

    #[test]
    fn primary_button_uses_utility_surface_text() {
        let t = tokens();
        assert_eq!(
            variant_colors(Variant::Primary),
            (t.primary, t.surface, t.primary)
        );
    }
}
