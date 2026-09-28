use iced::{Element, widget::button};

use crate::{
    config::Config,
    widgets::{
        bar_widget::Glyph,
        flat_button,
        phosphor::{Phosphor, Weight},
        widget_state::WidgetState,
    },
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
        let state = if self.armed {
            WidgetState::Pending
        } else {
            WidgetState::of_toggle(active)
        };
        let glyph = if self.armed {
            Phosphor::Warning(Weight::Regular)
        } else {
            Phosphor::Lightning(Weight::Regular)
        };
        let color = state.color(&config.palette).scale_alpha(opacity);
        button(Glyph::from(glyph.with_weight(state.weight())).view(config.font.icon_size_px, color))
            .padding(0)
            .style(flat_button(color))
            .on_press(on_press)
            .into()
    }
}
