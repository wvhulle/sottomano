use std::{
    future::{Future, ready},
    time::Duration,
};

use tokio::task::spawn_blocking;

use super::{
    Error, POWER_SUPPLY, battery::ChargeStatus, first_power_supply, parse_field, read_trimmed,
};
use crate::data::source::Source;

#[derive(Debug, Clone, PartialEq)]
pub struct PowerDraw {
    pub watts: f64,
    pub status: ChargeStatus,
}

impl Source for PowerDraw {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "power-consumption";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn available(_key: &()) -> impl Future<Output = bool> + Send {
        ready(first_power_supply("BAT").is_some())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_power_draw)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_power_draw() -> Result<PowerDraw, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    let uw: f64 = parse_field(&dir.join("power_now"))?;
    let status =
        read_trimmed(dir.join("status")).map_or(ChargeStatus::Unknown, |s| ChargeStatus::parse(&s));
    Ok(PowerDraw {
        watts: uw / 1_000_000.0,
        status,
    })
}
