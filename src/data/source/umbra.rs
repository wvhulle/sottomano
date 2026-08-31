use std::time::Duration;

use crate::{
    data::source::{BarWidget, Glyph, Source, TooltipBody, command, font_awesome::Solid, row},
    widgets::toggle_control::Toggle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Umbra {
    pub dark: bool,
}

impl BarWidget for Umbra {
    fn icon(&self) -> Glyph {
        if self.dark {
            Solid::Moon.into()
        } else {
            Solid::Sun.into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![row("Mode", if self.dark { "Dark" } else { "Light" })])
    }
}

impl Source for Umbra {
    type Error = command::Error;
    type Key = ();

    const NAME: &'static str = "umbra";
    const INTERVAL: Duration = Duration::from_secs(30);

    async fn available((): &()) -> bool {
        command::exists("umbra").await
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let mode = command::require("umbra", &["get"]).await?;
        Ok(Umbra {
            dark: mode.trim() == "dark",
        })
    }
}

impl Toggle for Umbra {
    fn toggled(&self) -> Self {
        Umbra { dark: !self.dark }
    }

    fn toggle(&self) -> bool {
        command::blocking::run("umbra", &["toggle"])
    }
}
