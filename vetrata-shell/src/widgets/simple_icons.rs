use iced::{Color, Element, widget::svg};
use simpleicons_rs::{SICLAUDE, SIGITHUBCOPILOT, SIOPENROUTER, SIUNSPLASH, SIWIKIPEDIA};

use crate::widgets::bar_widget::Glyph;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimpleIcon {
    Claude,
    OpenRouter,
    GithubCopilot,
    Wikipedia,
    Unsplash,
}

impl SimpleIcon {
    fn handle(self) -> svg::Handle {
        let data = match self {
            SimpleIcon::Claude => SICLAUDE.svg,
            SimpleIcon::OpenRouter => SIOPENROUTER.svg,
            SimpleIcon::GithubCopilot => SIGITHUBCOPILOT.svg,
            SimpleIcon::Wikipedia => SIWIKIPEDIA.svg,
            SimpleIcon::Unsplash => SIUNSPLASH.svg,
        };
        svg::Handle::from_memory(data.as_bytes())
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        svg(self.handle())
            .width(size)
            .height(size)
            .style(move |_, _| svg::Style { color: Some(color) })
            .into()
    }
}

impl From<SimpleIcon> for Glyph {
    fn from(icon: SimpleIcon) -> Self {
        Glyph::SimpleIcon(icon)
    }
}
