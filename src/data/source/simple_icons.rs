use iced::widget::svg;
use simpleicons_rs::{SICLAUDE, SIGITHUBCOPILOT, SIOPENROUTER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleIcon {
    Claude,
    OpenRouter,
    GithubCopilot,
}

impl SimpleIcon {
    pub fn handle(self) -> svg::Handle {
        let data = match self {
            SimpleIcon::Claude => SICLAUDE.svg,
            SimpleIcon::OpenRouter => SIOPENROUTER.svg,
            SimpleIcon::GithubCopilot => SIGITHUBCOPILOT.svg,
        };
        svg::Handle::from_memory(data.as_bytes())
    }
}
