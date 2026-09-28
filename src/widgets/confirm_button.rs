use iced::{Element, widget::button};

use crate::{
    config::Config,
    widgets::{flat_button, font_awesome::Solid},
};

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
        opacity: f32,
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
        let color = color.scale_alpha(opacity);
        button(glyph.view(config.font.icon_size_px, color))
            .padding(0)
            .style(flat_button(color))
            .on_press(on_press)
            .into()
    }
}
