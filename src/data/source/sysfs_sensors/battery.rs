use std::{
    future::{Future, ready},
    time::Duration,
};

use tokio::task::spawn_blocking;

use super::{Error, POWER_SUPPLY, energy_wh, first_power_supply, parse_field, read_field};
use crate::data::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl ChargeStatus {
    pub(super) fn parse(s: &str) -> Self {
        match s {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub capacity: u8,
    pub status: ChargeStatus,
    pub energy_wh: Option<f64>,
}

impl Source for Battery {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "battery";
    const INTERVAL: Duration = Duration::from_secs(30);

    fn available(_key: &()) -> impl Future<Output = bool> + Send {
        ready(first_power_supply("BAT").is_some())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_battery)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_battery() -> Result<Battery, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    Ok(Battery {
        capacity: parse_field(&dir.join("capacity"))?,
        status: ChargeStatus::parse(&read_field(&dir.join("status"))?),
        energy_wh: energy_wh(&dir),
    })
}
