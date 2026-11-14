use iced::{
    Color, Element, Theme,
    widget::{toggler, toggler::Status},
};

use crate::theming::{
    background_strong_color, background_weak_color, primary_color, primary_weak_color,
    secondary_text_color, text_color,
};

const DEFAULT_SWITCH_SIZE_PX: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TogglePosition {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchResponse {
    Pressable,
    Inert,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToggleSwitch {
    toggle_position: TogglePosition,
    response: SwitchResponse,
    size_px: f32,
    opacity: f32,
}

#[must_use]
pub fn toggle_switch(toggle_position: TogglePosition) -> ToggleSwitch {
    ToggleSwitch {
        toggle_position,
        response: SwitchResponse::Pressable,
        size_px: DEFAULT_SWITCH_SIZE_PX,
        opacity: 1.0,
    }
}

impl ToggleSwitch {
    #[must_use]
    pub fn size(self, size_px: f32) -> Self {
        Self { size_px, ..self }
    }

    #[must_use]
    pub fn opacity(self, opacity: f32) -> Self {
        Self { opacity, ..self }
    }

    #[must_use]
    pub fn response(self, response: SwitchResponse) -> Self {
        Self { response, ..self }
    }
}

impl<'a, Message: 'a> From<ToggleSwitch> for Element<'a, Message> {
    fn from(switch: ToggleSwitch) -> Self {
        let ToggleSwitch {
            toggle_position,
            response,
            size_px,
            opacity,
        } = switch;
        toggler(toggle_position == TogglePosition::On)
            .size(size_px)
            .style(move |theme, _status| switch_style(theme, toggle_position, response, opacity))
            .into()
    }
}

fn switch_style(
    theme: &Theme,
    toggle_position: TogglePosition,
    response: SwitchResponse,
    opacity: f32,
) -> toggler::Style {
    let knob_color = match response {
        SwitchResponse::Pressable => text_color(theme),
        SwitchResponse::Inert => secondary_text_color(theme),
    };
    toggler::Style {
        background: track_color(theme, toggle_position, response)
            .scale_alpha(opacity)
            .into(),
        foreground: knob_color.scale_alpha(opacity).into(),
        ..toggler::default(
            theme,
            Status::Active {
                is_toggled: toggle_position == TogglePosition::On,
            },
        )
    }
}

fn track_color(theme: &Theme, toggle_position: TogglePosition, response: SwitchResponse) -> Color {
    match response {
        SwitchResponse::Pressable => match toggle_position {
            TogglePosition::On => primary_color(theme),
            TogglePosition::Off => background_strong_color(theme),
        },
        SwitchResponse::Inert => match toggle_position {
            TogglePosition::On => primary_weak_color(theme),
            TogglePosition::Off => background_weak_color(theme),
        },
    }
}

#[cfg(test)]
mod tests {
    use iced::{
        Point, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::button,
    };
    use iced_test::Simulator;

    use super::*;

    #[test]
    fn a_press_on_the_switch_reaches_the_enclosing_item_button() {
        let pressable_switch: Element<'_, ()> = button(toggle_switch(TogglePosition::On))
            .padding(0)
            .on_press(())
            .into();
        let mut panel = Simulator::with_size(
            Settings::default(),
            Size::new(100.0, 100.0),
            pressable_switch,
        );
        let inside_switch = Point::new(5.0, 5.0);
        panel.point_at(inside_switch);
        let _moved_inside = panel.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: inside_switch,
        })]);
        let _pressed_and_released = panel.simulate([
            iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left)),
            iced::Event::Mouse(MouseEvent::ButtonReleased(Button::Left)),
        ]);
        assert_eq!(panel.into_messages().count(), 1);
    }

    #[test]
    fn the_track_differs_between_on_and_off() {
        assert_ne!(
            track_color(&Theme::Dark, TogglePosition::On, SwitchResponse::Pressable),
            track_color(&Theme::Dark, TogglePosition::Off, SwitchResponse::Pressable)
        );
    }

    #[test]
    fn an_inert_switch_draws_a_quieter_track_than_a_pressable_one() {
        assert_ne!(
            track_color(&Theme::Dark, TogglePosition::On, SwitchResponse::Inert),
            track_color(&Theme::Dark, TogglePosition::On, SwitchResponse::Pressable)
        );
    }
}
