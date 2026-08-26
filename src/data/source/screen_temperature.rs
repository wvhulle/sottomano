use std::time::Duration;

use crate::{
    data::source::{
        BarWidget, Glyph, Source, TooltipBody, command, font_awesome::Solid, gamma, row,
    },
    widgets::toggle_control::Toggle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SunFollow {
    pub auto: bool,
    pub unit: String,
}

impl BarWidget for SunFollow {
    fn icon(&self) -> Glyph {
        if self.auto {
            Solid::Sun.into()
        } else {
            Solid::Ban.into()
        }
    }

    fn disabled(&self) -> bool {
        !self.auto
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![row(
            "Auto-temperature",
            if self.auto { "on" } else { "off — manual" },
        )])
    }
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

impl Toggle for SunFollow {
    fn toggled(&self) -> Self {
        SunFollow {
            auto: !self.auto,
            unit: self.unit.clone(),
        }
    }

    fn toggle(&self) -> bool {
        if self.auto {
            stop(&self.unit)
        } else {
            restart(&self.unit)
        }
    }
}

fn stop(unit: &str) -> bool {
    command::blocking::run("systemctl", &["--user", "stop", unit])
}

fn restart(unit: &str) -> bool {
    command::blocking::run("systemctl", &["--user", "restart", unit])
}

pub fn set_manual(unit: &str, kelvin: u16) -> bool {
    stop(unit);
    gamma::set_temperature(kelvin)
}
