use iced::{Alignment, Element, widget::row};

use crate::press_actions::flat_element_button;

pub enum ConfirmPress {
    Fire,
    Armed,
}

pub struct ConfirmButtonIcons<'a, Message> {
    pub idle: Element<'a, Message>,
    pub confirm: Element<'a, Message>,
    pub cancel: Element<'a, Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmStage {
    Idle,
    Armed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmPolicy {
    ConfirmFirst,
    FireImmediately,
}

pub struct ConfirmButton {
    confirm_policy: ConfirmPolicy,
    stage: ConfirmStage,
}

impl ConfirmButton {
    #[must_use]
    pub fn new(confirm_policy: ConfirmPolicy) -> Self {
        Self {
            confirm_policy,
            stage: ConfirmStage::Idle,
        }
    }

    pub fn press(&mut self) -> ConfirmPress {
        if self.confirm_policy == ConfirmPolicy::ConfirmFirst && self.stage == ConfirmStage::Idle {
            self.stage = ConfirmStage::Armed;
            ConfirmPress::Armed
        } else {
            self.stage = ConfirmStage::Idle;
            ConfirmPress::Fire
        }
    }

    pub fn disarm(&mut self) {
        self.stage = ConfirmStage::Idle;
    }

    #[must_use]
    pub fn stage(&self) -> ConfirmStage {
        self.stage
    }

    pub fn view<'a, Message: 'a + Clone>(
        &self,
        icons: ConfirmButtonIcons<'a, Message>,
        spacing: f32,
        on_press: Message,
        on_cancel: Message,
    ) -> Element<'a, Message> {
        if self.stage == ConfirmStage::Idle {
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
