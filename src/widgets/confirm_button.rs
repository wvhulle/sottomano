use iced::{Alignment, Element, widget::row};

use crate::{
    config::Config,
    widgets::{
        glyph_button::GlyphButton,
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

    pub fn view<'a, Message: 'a + Clone>(
        &self,
        config: &Config,
        opacity: f32,
        active: bool,
        idle_glyph: Phosphor,
        on_press: Message,
        on_cancel: Message,
    ) -> Element<'a, Message> {
        if !self.armed {
            let state = WidgetState::of_toggle(active);
            return glyph_button(
                config,
                opacity,
                idle_glyph.with_weight(state.weight()),
                state,
                on_press,
            );
        }
        row![
            glyph_button(
                config,
                opacity,
                Phosphor::Check(Weight::Bold),
                WidgetState::Pending,
                on_press
            ),
            glyph_button(
                config,
                opacity,
                Phosphor::X(Weight::Regular),
                WidgetState::Off,
                on_cancel
            ),
        ]
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
    }
}

fn glyph_button<'a, Message: 'a + Clone>(
    config: &Config,
    opacity: f32,
    glyph: Phosphor,
    state: WidgetState,
    on_press: Message,
) -> Element<'a, Message> {
    let color = state.color(&config.theme.palette).scale_alpha(opacity);
    GlyphButton::new(glyph, config.theme.font.icon_size_px, color, on_press).into()
}
