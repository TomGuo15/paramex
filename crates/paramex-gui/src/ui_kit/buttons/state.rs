//! Button visual-state engine shared by button and segmented-control recipes.

use eframe::egui::{self, Color32, CornerRadius, Response, Stroke};

use crate::theme::{token_alpha, tokens};

use super::{
    Variant, BUTTON_CORNER_RADIUS, SECONDARY_BUTTON_HOVER_ALPHA, SECONDARY_BUTTON_PRESS_ALPHA,
    SEMANTIC_BUTTON_HOVER_ALPHA, SEMANTIC_BUTTON_PRESS_ALPHA,
};

/// Per-state `(fill, stroke)` pairs for a button. An explicit
/// `Button::fill`/`stroke` pins every state ("this will override any on-hover
/// effects" per egui docs), so the variant buttons gave no hover/press feedback.
/// The pairs land in scoped widget visuals instead: egui's `button_style`
/// reads frame fill from `weak_bg_fill` and stroke from `bg_stroke` per state.
#[derive(Clone, Copy)]
pub(in crate::ui_kit) struct ButtonStates {
    rest: (Color32, Color32),
    hover: (Color32, Color32),
    press: (Color32, Color32),
}

/// States for a FILLED button (Primary, the direction-coloured buttons):
/// hover keeps the fill but swaps to a contrasting approved-token stroke;
/// press lands on the Studio Stellar dark token.
pub(in crate::ui_kit) fn filled_states(color: Color32) -> ButtonStates {
    let t = tokens();
    let hover_stroke = if color == t.ink { t.primary } else { t.ink };
    ButtonStates {
        rest: (color, color),
        hover: (color, hover_stroke),
        press: (t.ink, t.ink),
    }
}

/// States for an OUTLINED button (white fill, hue text/stroke): hover/press use
/// explicit alpha washes of the button's own palette token over the card.
pub(in crate::ui_kit) fn outlined_states(
    rest_stroke: Color32,
    hue: Color32,
    hover_alpha: u8,
    press_alpha: u8,
) -> ButtonStates {
    ButtonStates {
        rest: (tokens().surface, rest_stroke),
        hover: (token_alpha(hue, hover_alpha), hue),
        press: (token_alpha(hue, press_alpha), hue),
    }
}

/// States for a soft semantic filled button (Warning). Text stays Studio dark,
/// so press/hover feedback uses only approved-token stroke changes.
fn soft_semantic_filled_states(fill: Color32) -> ButtonStates {
    let t = tokens();
    ButtonStates {
        rest: (fill, fill),
        hover: (fill, t.ink),
        press: (fill, t.primary),
    }
}

/// The rest pairs must equal `variant_colors`' fill/stroke exactly; the at-rest
/// render is contractually unchanged by the state engine.
fn variant_states(v: Variant) -> ButtonStates {
    let t = tokens();
    match v {
        Variant::Primary => filled_states(t.primary),
        // Secondary rests on the neutral `border` hairline and only takes its
        // brand outline on hover (mirrors the theme-wide hover language).
        Variant::Secondary => outlined_states(
            t.border,
            t.primary,
            SECONDARY_BUTTON_HOVER_ALPHA,
            SECONDARY_BUTTON_PRESS_ALPHA,
        ),
        Variant::Danger => outlined_states(
            t.red,
            t.red,
            SEMANTIC_BUTTON_HOVER_ALPHA,
            SEMANTIC_BUTTON_PRESS_ALPHA,
        ),
        Variant::Warning => soft_semantic_filled_states(t.yellow),
    }
}

/// Like [`variant_states`] but honoring the host ui's enabled state: a disabled
/// button keeps the white/grey rest pair in every state (no hover
/// feedback on an inert control).
pub(in crate::ui_kit) fn variant_states_in(ui: &egui::Ui, v: Variant) -> ButtonStates {
    if ui.is_enabled() {
        variant_states(v)
    } else {
        let t = tokens();
        ButtonStates {
            rest: (t.surface, t.ink_soft),
            hover: (t.surface, t.ink_soft),
            press: (t.surface, t.ink_soft),
        }
    }
}

/// Add `content` as a button whose fill/stroke come from `states` via scoped
/// widget visuals (7px corners, the shared 30px-min-height chain). `size`
/// pins the slot (full-width buttons); `None` is content-width.
pub(in crate::ui_kit) fn add_state_button(
    ui: &mut egui::Ui,
    content: impl egui::IntoAtoms<'static>,
    states: ButtonStates,
    size: Option<egui::Vec2>,
    min_size: egui::Vec2,
) -> Response {
    ui.scope(|ui| {
        fn set(wv: &mut egui::style::WidgetVisuals, (fill, stroke): (Color32, Color32)) {
            wv.weak_bg_fill = fill;
            wv.bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0_f32, stroke);
            wv.corner_radius = CornerRadius::same(BUTTON_CORNER_RADIUS);
        }
        let widgets = &mut ui.style_mut().visuals.widgets;
        set(&mut widgets.noninteractive, states.rest);
        set(&mut widgets.inactive, states.rest);
        set(&mut widgets.hovered, states.hover);
        set(&mut widgets.active, states.press);
        let btn = egui::Button::new(content).min_size(min_size);
        match size {
            Some(s) => ui.add_sized(s, btn),
            None => ui.add(btn),
        }
    })
    .inner
}
