use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Element, Subscription};

use crate::{user_config::RuntimeConfig, widgets::text::bar_text};

pub struct ClockGroup {
    text: String,
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self {
            text: format_hms(Local::now()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage) {
        let ClockMessage::Tick(now) = msg;
        self.text = format_hms(now);
    }

    pub fn subscription() -> Subscription<ClockMessage> {
        iced::time::every(Duration::from_secs(1)).map(|_| ClockMessage::Tick(Local::now()))
    }

    pub fn view<'a>(&self, config: &RuntimeConfig) -> Element<'a, ClockMessage> {
        bar_text(&config.font, self.text.clone()).into()
    }
}

fn format_hms(now: DateTime<Local>) -> String {
    now.format("%H:%M:%S").to_string()
}
