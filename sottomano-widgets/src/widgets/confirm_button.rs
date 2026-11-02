use iced::{Alignment, Element, Theme, widget::row};
use sottomano_config::Config;

use crate::widgets::{
    glyph_button,
    phosphor::{Phosphor, Weight},
    reading_display_state::{ReadingDisplayState, ReadingStateTint},
    source_reading::icon_value_gap,
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
    #[must_use]
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

    #[must_use]
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
        let color = |state: ReadingDisplayState| {
            let tint = ReadingStateTint { state, opacity };
            move |theme: &Theme| tint.color(theme)
        };
        if !self.armed {
            let state = ReadingDisplayState::of_toggle(active);
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
                color(ReadingDisplayState::Pending),
                on_press
            ),
            glyph_button(
                Phosphor::X(Weight::Regular),
                size,
                color(ReadingDisplayState::Off),
                on_cancel
            ),
        ]
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
    }
}
