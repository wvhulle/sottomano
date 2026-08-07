use iced::{Element, widget::mouse_area};

use crate::{config::Config, data::source::font_awesome::Solid};

pub enum Press {
    Fire,
    Armed,
}

pub struct ConfirmButton {
    confirms: bool,
    armed: bool,
}

impl ConfirmButton {
    pub fn new(confirms: bool) -> Self {
        Self {
            confirms,
            armed: false,
        }
    }

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

    pub fn view<'a, Message: 'a + Clone>(
        &self,
        config: &Config,
        alpha: f32,
        active: bool,
        on_press: Message,
    ) -> Element<'a, Message> {
        let (glyph, color) = if self.armed {
            (Solid::TriangleExclamation, config.palette.urgent)
        } else if active {
            (Solid::Bolt, config.palette.bar_foreground)
        } else {
            (Solid::Bolt, config.palette.inactive)
        };
        let color = color.scale_alpha(alpha);
        mouse_area(glyph.widget().size(config.font.icon_size_px).color(color))
            .on_press(on_press)
            .into()
    }
}
