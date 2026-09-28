use iced::{
    Color, Element,
    widget::{container, svg},
};
use simpleicons_rs::{SICLAUDE, SIGITHUBCOPILOT, SIOPENROUTER, SIUNSPLASH, SIWIKIPEDIA};

use crate::widgets::bar_widget::Glyph;

const PHOSPHOR_LIVE_AREA_FRACTION: f32 = 224.0 / 256.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        let drawing = size * PHOSPHOR_LIVE_AREA_FRACTION;
        container(
            svg(self.handle())
                .width(drawing)
                .height(drawing)
                .style(move |_, _| svg::Style { color: Some(color) }),
        )
        .center(size)
        .into()
    }
}

impl From<SimpleIcon> for Glyph {
    fn from(icon: SimpleIcon) -> Self {
        Glyph::SimpleIcon(icon)
    }
}
