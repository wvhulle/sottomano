use iced::{Alignment, Element, widget::row};

use crate::flat_button::flat_element_button;

pub enum Press {
    Fire,
    Armed,
}

pub struct ConfirmButtonIcons<'a, Message> {
    pub idle: Element<'a, Message>,
    pub confirm: Element<'a, Message>,
    pub cancel: Element<'a, Message>,
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
        icons: ConfirmButtonIcons<'a, Message>,
        spacing: f32,
        on_press: Message,
        on_cancel: Message,
    ) -> Element<'a, Message> {
        if !self.armed {
            return flat_element_button(icons.idle, on_press);
        }
        row![
            flat_element_button(icons.confirm, on_press),
            flat_element_button(icons.cancel, on_cancel),
        ]
        .spacing(spacing)
        .align_y(Alignment::Center)
        .into()
    }
}
