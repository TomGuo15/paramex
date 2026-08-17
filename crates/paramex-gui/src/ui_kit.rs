//! Reusable design-system primitives: white cards with a soft shadow, compact
//! single-row title rails, flat local tabs/actions, right-aligned numeric inputs,
//! and the standard body-button variants. All colours come from [`crate::theme`].
//!
//! Header titles may contain `<sub>`/`<sup>` markup (rendered via [`crate::richtext`]),
//! so e.g. `"GATE OXIDE C<sub>ox</sub>"` renders correctly. Titles are passed in their
//! final case (no auto-uppercasing -- that would corrupt the markup tags).

use eframe::egui::{self, FontFamily};

mod buttons;
mod cards;
mod headers;
mod inputs;
mod metrics;
mod scroll;
mod segments;
mod selection;
mod sliders;
mod status;
mod text;

pub use buttons::*;
pub use cards::*;
pub use headers::*;
pub use inputs::*;
pub use metrics::*;
pub use scroll::*;
pub use segments::*;
pub use selection::*;
pub use sliders::*;
pub use status::*;
pub use text::*;

/// Per-context flag (set by `theme::install`) marking that the `"bold"` font family
/// has been registered. Lets `bold_family` fall back safely in tests that render a
/// panel without installing the theme (egui panics on an unbound `Name` family).
pub(crate) const BOLD_READY_FLAG: &str = "paramex_bold_font_ready";

/// The custom bold font family -- only if `theme::install` registered it on this
/// context; otherwise the proportional font (so un-themed harness tests don't panic).
pub fn bold_family(ui: &egui::Ui) -> FontFamily {
    let ready = ui.ctx().data(|d| {
        d.get_temp::<bool>(egui::Id::new(BOLD_READY_FLAG))
            .unwrap_or(false)
    });
    if ready {
        FontFamily::Name("bold".into())
    } else {
        FontFamily::Proportional
    }
}
