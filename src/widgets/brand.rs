use iced::widget::svg;
use simpleicons_rs::{SICLAUDE, SIGITHUBCOPILOT, SIOPENROUTER};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandMark {
    Claude,
    OpenRouter,
    GithubCopilot,
}

impl BrandMark {
    pub fn handle(self) -> svg::Handle {
        let data = match self {
            BrandMark::Claude => SICLAUDE.svg,
            BrandMark::OpenRouter => SIOPENROUTER.svg,
            BrandMark::GithubCopilot => SIGITHUBCOPILOT.svg,
        };
        svg::Handle::from_memory(data.as_bytes())
    }
}
