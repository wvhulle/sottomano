use iced::{Alignment, Element, widget::row};

use crate::{
    config::Config,
    widgets::{
        glyph_button,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
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

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn view<'a, Message: 'a + Clone>(
        &self,
        config: &Config,
        opacity: f32,
        active: bool,
        idle_glyph: Phosphor,
        on_press: Message,
        on_cancel: Message,
    ) -> Element<'a, Message> {
        let size = config.theme.font.icon_size_px;
        let color = |state: WidgetState| state.color(&config.theme.palette).scale_alpha(opacity);
        if !self.armed {
            let state = WidgetState::of_toggle(active);
            return glyph_button(
                idle_glyph.with_weight(state.weight()),
                size,
                color(state),
                on_press,
            );
        }
        row![
            glyph_button(
                Phosphor::Check(Weight::Bold),
                size,
                color(WidgetState::Pending),
                on_press
            ),
            glyph_button(
                Phosphor::X(Weight::Regular),
                size,
                color(WidgetState::Off),
                on_cancel
            ),
        ]
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
    }
}
