use std::time::Duration;

use crate::data::source::{Source, command};

#[derive(Debug, Clone, PartialEq)]
pub struct Umbra {
    pub dark: bool,
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

pub fn toggle_mode() -> bool {
    command::blocking::run("umbra", &["toggle"])
}
