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
        TooltipBody::Table(vec![row("Mode", if self.dark { "Dark" } else { "Light" })])
    }
}

impl Source for Umbra {
    type Error = command::Error;

    const NAME: &'static str = "umbra";
    const INTERVAL: Duration = Duration::from_secs(30);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        Ok(command::output("umbra", &["get"]).await?.map(|mode| Umbra {
            dark: mode.trim() == "dark",
        }))
    }
}

impl Toggle for Umbra {
    fn toggled(&self) -> Self {
        Umbra { dark: !self.dark }
    }

    fn toggle() -> bool {
        command::blocking::run("umbra", &["toggle"])
    }
}
