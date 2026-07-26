//! Dark/light mode via odin's `umbra` CLI. The indicator shows a moon (dark) or
//! sun (light); clicking runs `umbra toggle`. `None` when `umbra` is not on
//! PATH, which hides the toggle.
//!
//! odin's widget also renders a heliocron countdown to the next sunrise/sunset
//! switch; that label is deferred — the icon and mode tooltip come first.

use std::time::Duration;

use crate::{
    data_sources::{BarWidget, Source, TooltipBody, command, row},
    widgets::{icon::Icon, toggle_control::Toggle},
};

#[derive(Debug, Clone)]
pub struct Umbra {
    pub dark: bool,
}

impl BarWidget for Umbra {
    fn icon(&self) -> Icon {
        if self.dark { Icon::Moon } else { Icon::Sun }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![row("Mode", if self.dark { "Dark" } else { "Light" })])
    }
}

impl Source for Umbra {
    const NAME: &'static str = "umbra";
    const INTERVAL: Duration = Duration::from_secs(30);

    fn poll() -> Option<Self> {
        let mode = command::output("umbra", &["get"])?;
        Some(Umbra {
            dark: mode.trim() == "dark",
        })
    }
}

impl Toggle for Umbra {
    fn toggled(&self) -> Self {
        Umbra { dark: !self.dark }
    }

    fn toggle() -> bool {
        command::run("umbra", &["toggle"])
    }
}
