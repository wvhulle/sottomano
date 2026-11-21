use iced::{Alignment, Element, widget::row};

use crate::press_actions::flat_element_button;

pub enum ConfirmPress {
    Fire,
    Armed,
}

pub struct ConfirmButtonIcons<'view, Message> {
    pub idle: Element<'view, Message>,
    pub confirm: Element<'view, Message>,
    pub cancel: Element<'view, Message>,
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
    pub stage: ConfirmStage,
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

    pub fn view<'view, Message: 'view + Clone>(
        &self,
        icons: ConfirmButtonIcons<'view, Message>,
        spacing: f32,
        on_press: Message,
        on_cancel: Message,
    ) -> Element<'view, Message> {
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
