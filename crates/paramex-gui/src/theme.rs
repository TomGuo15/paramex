//! ParamEx theme: shared colors/visuals, the system Segoe UI font, and the window
//! icon. Flat fills keep the body clear without gradients or backdrop blur.

mod install;

use eframe::egui::{self, Color32};

pub use install::{install, window_icon};

/// The design-system palette. Base colors come only from the user-approved
/// groups: Studio Stellar for the app theme, ReGLOSS Text for semantic accents,
/// and Utility for neutral surfaces/text. State fills use alpha variants of
/// those tokens instead of introducing new hues.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub ink: Color32,
    pub ink_soft: Color32,
    pub primary: Color32,
    /// Selected-state fill: `primary` at low alpha (rows, active tabs).
    pub accent_soft: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub red: Color32,
    pub bg: Color32,
    /// Studio Stellar light. Cards also carry an inner bezel and soft shadow, so
    /// the shell stays separated without introducing another border hue.
    pub border: Color32,
    pub surface: Color32,
    pub surface_muted: Color32,
}

pub const SUISEI_DARK: Color32 = Color32::from_rgb(0x00, 0x0F, 0x30);
pub const SUISEI_MAIN: Color32 = Color32::from_rgb(0x00, 0x3C, 0xFF);
pub const SUISEI_LIGHT: Color32 = Color32::from_rgb(0xCD, 0xDE, 0xE5);

pub const KANADE_GOLD_PASTEL: Color32 = Color32::from_rgb(0xF8, 0xE7, 0xB9);
pub const RIRIKA_PINK_PASTEL: Color32 = Color32::from_rgb(0xDF, 0x83, 0xA7);
pub const RADEN_GREEN_PASTEL: Color32 = Color32::from_rgb(0x9B, 0xE5, 0xD4);

pub const UTILITY_WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const UTILITY_BLACK: Color32 = Color32::from_rgb(0x00, 0x00, 0x00);
pub const UTILITY_CHARCOAL: Color32 = Color32::from_rgb(0x60, 0x60, 0x60);
pub const UTILITY_GRAY: Color32 = Color32::from_rgb(0x88, 0x88, 0x88);

pub const ACCENT_SOFT_ALPHA: u8 = 31;
pub const SELECTION_FILL_ALPHA: u8 = 64;
pub const ACTIVE_WIDGET_FILL_ALPHA: u8 = 46;

pub fn utility_white_alpha(alpha: u8) -> Color32 {
    Color32::from_white_alpha(alpha)
}

pub fn utility_black_alpha(alpha: u8) -> Color32 {
    Color32::from_black_alpha(alpha)
}

pub fn token_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn tokens() -> Tokens {
    let primary = SUISEI_MAIN;
    Tokens {
        ink: SUISEI_DARK,
        ink_soft: UTILITY_CHARCOAL,
        primary,
        accent_soft: token_alpha(primary, ACCENT_SOFT_ALPHA),
        green: RADEN_GREEN_PASTEL,
        yellow: KANADE_GOLD_PASTEL,
        red: RIRIKA_PINK_PASTEL,
        bg: SUISEI_LIGHT,
        border: SUISEI_LIGHT,
        surface: UTILITY_WHITE,
        surface_muted: SUISEI_LIGHT,
    }
}

/// Neutral raised surfaces on the dark banner: the logo mark panel
/// (`brand_bar`) and the Banner segmented track (`ui_kit::segmented`).
pub const INK_RAISED: Color32 = UTILITY_BLACK;

/// The one soft elevation shadow: cards (`ui_kit::card_frame`) and the hover
/// chrome (tooltips, popups, menus) share it so everything floats at the same
/// height — egui's default popup shadow is darker/tighter and read foreign
/// next to the cards.
pub fn soft_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: utility_black_alpha(15),
    }
}

/// Clip headroom for surfaces casting [`soft_shadow`]: the shadow's farthest
/// reach past the rect (|offset| + spread + blur/2, per epaint `Shadow::margin`)
/// plus slack. Card slots and shell columns expand their clip rects by this so
/// the shadow is never sheared at a seam — derived from the shadow itself so
/// retuning it can't strand a hand-copied constant.
pub fn soft_shadow_clearance() -> f32 {
    let s = soft_shadow();
    let reach = s.offset[0].abs().max(s.offset[1].abs()) as f32
        + f32::from(s.spread)
        + f32::from(s.blur) / 2.0;
    reach + 6.0
}
