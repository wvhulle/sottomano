use std::time::Duration;

use crate::data::source::{Source, command, gamma};

#[derive(Debug, Clone, PartialEq)]
pub struct SunFollow {
    pub auto: bool,
    pub unit: String,
}

impl Source for SunFollow {
    type Error = command::Error;
    type Key = String;

    const NAME: &'static str = "screen temperature";
    const INTERVAL: Duration = Duration::from_secs(10);

    async fn available(_unit: &String) -> bool {
        command::exists("systemctl").await
    }

    async fn poll(unit: String) -> Result<Self, Self::Error> {
        let state = command::require(
            "systemctl",
            &["--user", "show", "-p", "ActiveState", "--value", &unit],
        )
        .await?;
        Ok(SunFollow {
            auto: state.trim() == "active",
            unit,
        })
    }
}

pub fn stop(unit: &str) -> bool {
    command::blocking::run("systemctl", &["--user", "stop", unit])
}

pub fn restart(unit: &str) -> bool {
    command::blocking::run("systemctl", &["--user", "restart", unit])
}

pub fn set_manual(unit: &str, kelvin: u16) -> bool {
    stop(unit);
    gamma::set_temperature(kelvin)
}
