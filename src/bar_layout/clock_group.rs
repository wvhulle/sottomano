//! The centered clock: a single-widget static group. Owns its own ticking text.

use chrono::Local;
use iced::Element;

use crate::{bar_program::Message, user_config::Config, visual_elements::bar_text};

/// A single-widget static group holding the centered clock text.
pub struct ClockGroup {
    text: String,
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self { text: now_hm() }
    }
}

impl ClockGroup {
    /// Refresh the displayed text from the local-time clock.
    pub fn tick(&mut self) {
        self.text = now_hm();
    }

    /// The clock as a centered text widget.
    pub fn view<'a>(&self, _config: &Config) -> Element<'a, Message> {
        bar_text(self.text.clone()).into()
    }
}

/// Local-time `HH:MM` in 24-hour form. Re-evaluated on every
/// [`ClockGroup::tick`].
fn now_hm() -> String {
    Local::now().format("%H:%M").to_string()
}
