use iced::{
    Color, Element, Theme,
    widget::{toggler, toggler::Status},
};

use crate::theme_colors::{background_color, primary_color, secondary_text_color};

#[must_use]
pub fn toggle_switch<'a, Message: 'a>(
    size_px: f32,
    switched_on: bool,
    opacity: f32,
) -> Element<'a, Message> {
    toggler(switched_on)
        .size(size_px)
        .style(move |theme, _status| switch_style(theme, switched_on, opacity))
        .into()
}

fn switch_style(theme: &Theme, switched_on: bool, opacity: f32) -> toggler::Style {
    toggler::Style {
        background: track_color(theme, switched_on).scale_alpha(opacity).into(),
        foreground: background_color(theme).scale_alpha(opacity).into(),
        ..toggler::default(
            theme,
            Status::Active {
                is_toggled: switched_on,
            },
        )
    }
}

fn track_color(theme: &Theme, switched_on: bool) -> Color {
    if switched_on {
        primary_color(theme)
    } else {
        secondary_text_color(theme)
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

    const SWITCH_SIZE_PX: f32 = 16.0;

    #[test]
    fn a_press_on_the_switch_reaches_the_enclosing_item_button() {
        let pressable_switch: Element<'_, ()> = button(toggle_switch(SWITCH_SIZE_PX, true, 1.0))
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
            track_color(&Theme::Dark, true),
            track_color(&Theme::Dark, false)
        );
    }
}
