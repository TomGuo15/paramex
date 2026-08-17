//! Top brand bar (`shell/page.py:28-52`): the painter logo mark, the "ParamEx"
//! name + the Transfer/TLM workspace toggle, and the 7-click Suisei easter egg (the
//! comet mark goes gold). The "Suisei mode" toast that used to also fire was removed
//! (user pref) — the gold comet is the only feedback now.

use eframe::egui::{self, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::state::{EasterEgg, Workspace};
use crate::theme;
use crate::ui_kit;

const BANNER_WORDMARK_SIZE: f32 = 15.0;
const BRAND_MARK_WIDTH: f32 = 58.0;
const BRAND_MARK_HEIGHT: f32 = 34.0;
const BRAND_MARK_CORNER_RADIUS: u8 = 8;
const BRAND_MARK_BORDER_ALPHA: u8 = 28;
const BRAND_LOGO_PRIMARY_WIDTH: f32 = 2.0;
const BRAND_LOGO_CONNECTOR_WIDTH: f32 = 1.5;
const BRAND_LOGO_GUIDE_WIDTH: f32 = 1.0;
const BRAND_LOGO_DOT_RADIUS: f32 = 1.6;
const BRAND_LOGO_CONNECTOR_ALPHA: u8 = 76;
const BRAND_LOGO_GUIDE_ALPHA: u8 = 114;
const BRAND_LOGO_DOT_SOFT_ALPHA: u8 = 102;
const BRAND_LOGO_DOT_STRONG_ALPHA: u8 = 216;
const SUISEI_WATERMARK_ALPHA: u8 = 48;
const BANNER_ACTION_SIZE: f32 = 24.0;
const BANNER_ACTION_REST_ALPHA: u8 = 150;
const BANNER_ACTION_HOVER_ALPHA: u8 = 230;

/// Draw the brand bar. `egg` is the easter-egg counter; the 7th logo click toggles
/// the gold Suisei comet on the right. `active` is the current workspace — the
/// `[Transfer][TLM]` toggle reads + writes it.
pub fn show(ui: &mut egui::Ui, egg: &mut EasterEgg, active: &mut Workspace, show_help: &mut bool) {
    ui.add_space(10.0);
    ui.horizontal_centered(|ui| {
        ui.add_space(6.0);
        // The 58×34 brand mark sits on the dark bar with a subtle white hairline
        // so the white logo art reads against the ink banner.
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(BRAND_MARK_WIDTH, BRAND_MARK_HEIGHT),
            Sense::click(),
        );
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, BRAND_MARK_CORNER_RADIUS, theme::INK_RAISED);
        painter.rect_stroke(
            rect.shrink(0.5),
            BRAND_MARK_CORNER_RADIUS,
            Stroke::new(1.0_f32, theme::utility_white_alpha(BRAND_MARK_BORDER_ALPHA)),
            StrokeKind::Inside,
        );
        draw_logo(&painter, rect);
        if resp.clicked() {
            // 7th click toggles the gold comet (the easter-egg payoff). The "Suisei mode"
            // toast that used to also fire here was removed per user preference — the gold
            // comet on the right is the only feedback now ("suisei mode still has its use").
            egg.register_click();
        }

        ui.add_space(10.0);
        // White name + the workspace toggle, legible on the dark banner.
        ui.label(
            RichText::new("ParamEx")
                .color(theme::tokens().surface)
                .size(BANNER_WORDMARK_SIZE)
                .strong(),
        );
        ui.add_space(8.0);
        // The workspace toggle: white lifted pill = active, transparent + white label =
        // inactive, both on the black raised track.
        if let Some(idx) = ui_kit::segmented(
            ui,
            &["Transfer", "TLM", "Model Fit"],
            active.index(),
            ui_kit::SegStyle::Banner,
            Some(86.0),
        ) {
            *active = Workspace::from_index(idx);
        }

        // The Suisei comet mark pinned to the right of the bar (page.py
        // `.paramex-suisei-mark`): a faint watermark normally, gold when the 7-click
        // easter egg is active. (The bar background is painted full-width by `app.rs`,
        // so this right-aligned block only places the comet — it no longer drives the
        // banner width.)
        ui_kit::right_aligned(ui, |ui| {
            ui.add_space(8.0);
            if banner_action(ui, "Data guide").clicked() {
                *show_help = true;
            }
            ui.add_space(8.0);
            let tex = suisei_texture(ui.ctx());
            let h = 24.0;
            let size = Vec2::new(h * 142.0 / 88.0, h);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let tint = if egg.is_shown() {
                theme::tokens().yellow
            } else {
                theme::utility_white_alpha(SUISEI_WATERMARK_ALPHA)
            };
            ui.painter().image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                tint,
            );
        });
    });
}

fn banner_action(ui: &mut egui::Ui, label: &str) -> Response {
    let enabled = ui.is_enabled();
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(egui::Vec2::splat(BANNER_ACTION_SIZE), sense);
    let alpha = if response.is_pointer_button_down_on() {
        255
    } else if response.hovered() || response.has_focus() {
        BANNER_ACTION_HOVER_ALPHA
    } else {
        BANNER_ACTION_REST_ALPHA
    };
    let color = theme::utility_white_alpha(alpha);
    ui.painter()
        .circle_stroke(rect.center(), 9.0, Stroke::new(1.4_f32, color));
    ui.painter().text(
        rect.center() + egui::vec2(0.0, -0.5),
        egui::Align2::CENTER_CENTER,
        "?",
        FontId::new(12.0, ui_kit::bold_family(ui)),
        color,
    );
    if enabled {
        response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(label);
    }
    let owned = label.to_owned();
    response.widget_info(move || {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, owned.clone())
    });
    response
}

/// Load (and cache per-context) the Suisei comet PNG as a texture. White on
/// transparent, so callers tint it.
fn suisei_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("paramex_suisei_tex");
    if let Some(handle) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return handle;
    }
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../assets/suisei.png"))
        .expect("baked suisei.png decodes");
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    let handle = ctx.load_texture("suisei", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}

/// Faithful realization of `logo.py` `PARAMEX_LOGO_SVG` (viewBox 0 0 100 62):
/// 7 scatter dots, the bold central fit line, two dashed verticals — white on the
/// dark mark. Coordinates are the SVG's, scaled into the mark's inner area.
fn draw_logo(painter: &egui::Painter, rect: Rect) {
    // SVG viewBox is 100×62; inset the mark a little (base.css padding 4×8).
    let inset = Vec2::new(8.0, 4.0);
    let area = Rect::from_min_max(rect.min + inset, rect.max - inset);
    let sx = area.width() / 100.0;
    let sy = area.height() / 62.0;
    let p = |x: f32, y: f32| Pos2::new(area.min.x + x * sx, area.min.y + y * sy);
    let primary_stroke = Stroke::new(BRAND_LOGO_PRIMARY_WIDTH, theme::utility_white_alpha(255));
    let connector_stroke = Stroke::new(
        BRAND_LOGO_CONNECTOR_WIDTH,
        theme::utility_white_alpha(BRAND_LOGO_CONNECTOR_ALPHA),
    );
    let guide_stroke = Stroke::new(
        BRAND_LOGO_GUIDE_WIDTH,
        theme::utility_white_alpha(BRAND_LOGO_GUIDE_ALPHA),
    );
    // Bold central fit line (30,43)->(70,18).
    painter.line_segment([p(30.0, 43.0), p(70.0, 18.0)], primary_stroke);
    // Faint connector segments.
    painter.line_segment([p(8.0, 56.0), p(30.0, 43.0)], connector_stroke);
    painter.line_segment([p(70.0, 18.0), p(92.0, 5.0)], connector_stroke);
    // Two dashed verticals at x=30 and x=70 (drawn as short ticks for simplicity).
    for x in [30.0_f32, 70.0] {
        painter.line_segment([p(x, 6.0), p(x, 57.0)], guide_stroke);
    }
    // Scatter dots (cx,cy,opacity) from the SVG.
    for (cx, cy, a) in [
        (8.0, 56.0, 0.4),
        (23.0, 47.0, 0.4),
        (38.0, 38.0, 0.85),
        (53.0, 29.0, 0.85),
        (68.0, 20.0, 0.85),
        (77.0, 14.0, 0.4),
        (92.0, 5.0, 0.4),
    ] {
        let alpha = if a > 0.5 {
            BRAND_LOGO_DOT_STRONG_ALPHA
        } else {
            BRAND_LOGO_DOT_SOFT_ALPHA
        };
        painter.circle_filled(
            p(cx, cy),
            BRAND_LOGO_DOT_RADIUS,
            theme::utility_white_alpha(alpha),
        );
    }
}
