use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Element, Subscription, Task};

use crate::{bar_program::Message, user_config::Config, widgets::bar_text};

pub struct ClockGroup {
    text: String,
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self {
            text: format_hm(Local::now()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage) -> Task<Message> {
        let ClockMessage::Tick(now) = msg;
        self.text = format_hm(now);
        Task::none()
    }

    pub fn subscription() -> Subscription<Message> {
        iced::time::every(Duration::from_secs(1))
            .map(|_| Message::Clock(ClockMessage::Tick(Local::now())))
    }

    pub fn view<'a>(&self, _config: &Config) -> Element<'a, Message> {
        bar_text(self.text.clone()).into()
    }
}

fn format_hm(now: DateTime<Local>) -> String {
    now.format("%H:%M").to_string()
}
