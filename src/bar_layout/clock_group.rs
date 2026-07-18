//! The centered clock: a single-widget static group. Owns its own ticking text.

use iced::widget::text::Shaping;
use iced::widget::text;
use iced::Element;

use crate::bar_program::Message;
use crate::user_config::Config;
use crate::visual_elements::font_family;

pub struct ClockGroup {
    text: String,
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self { text: now_hm() }
    }
}

impl ClockGroup {
    pub fn tick(&mut self) {
        self.text = now_hm();
    }

    pub fn view<'a>(&self, config: &Config) -> Element<'a, Message> {
        let size = config.font.size as f32;
        text(self.text.clone())
            .size(size)
            .font(font_family(&config.font.family))
            .shaping(Shaping::Advanced)
            .into()
    }
}

fn now_hm() -> String {
    // Minimal, dependency-free HH:MM from the system clock (UTC). A real bar would use
    // chrono with the local timezone; the spike only needs a ticking value.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    format!("{h:02}:{m:02}")
}
