//! A bar action button with an optional two-step confirm. A harmless action
//! fires on a single click; a disruptive one arms on the first click — its play
//! glyph turning to a warning in the urgent colour — and only fires on a
//! second. The button owns just the armed flag and its rendering; the parent
//! drives it by click and disarms it on any competing interaction (there is no
//! timer).

use iced::{Color, Element, widget::mouse_area};

use crate::{config::resolved::ResolvedConfig, widgets::icon::Icon};

/// The outcome of a click: fire the action now, or (first click on a
/// confirm-gated button) arm and wait for the confirming second click.
pub enum Press {
    Fire,
    Armed,
}

pub struct ConfirmButton {
    confirms: bool,
    armed: bool,
}

impl ConfirmButton {
    /// `confirms`: whether this action needs the two-step confirm.
    pub fn new(confirms: bool) -> Self {
        Self {
            confirms,
            armed: false,
        }
    }

    /// Register a click: fire (no confirm needed, or already armed) or arm.
    pub fn press(&mut self) -> Press {
        if self.confirms && !self.armed {
            self.armed = true;
            Press::Armed
        } else {
            self.armed = false;
            Press::Fire
        }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// The button glyph. Armed → warning in `urgent`; else play in `focus`.
    /// `alpha` fades the whole glyph (e.g. by a reveal factor); `on_press` is
    /// the message emitted on click.
    pub fn view<'a, Message: 'a + Clone>(
        &self,
        config: &ResolvedConfig,
        alpha: f32,
        on_press: Message,
    ) -> Element<'a, Message> {
        let (glyph, color) = if self.armed {
            (Icon::TriangleExclamation, config.palette.urgent)
        } else {
            (Icon::Bolt, config.palette.bar_foreground)
        };
        let color = Color {
            a: color.a * alpha,
            ..color
        };
        mouse_area(glyph.solid().size(config.font.icon_size_px).color(color))
            .on_press(on_press)
            .into()
    }
}
