use iced::widget::svg;
use simpleicons_rs::{SICLAUDE, SIGITHUBCOPILOT, SIOPENROUTER, SIUNSPLASH, SIWIKIPEDIA};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleIcon {
    Claude,
    OpenRouter,
    GithubCopilot,
    Wikipedia,
    Unsplash,
}

impl SimpleIcon {
    pub fn handle(self) -> svg::Handle {
        let data = match self {
            SimpleIcon::Claude => SICLAUDE.svg,
            SimpleIcon::OpenRouter => SIOPENROUTER.svg,
            SimpleIcon::GithubCopilot => SIGITHUBCOPILOT.svg,
            SimpleIcon::Wikipedia => SIWIKIPEDIA.svg,
            SimpleIcon::Unsplash => SIUNSPLASH.svg,
        };
        svg::Handle::from_memory(data.as_bytes())
    }
}
