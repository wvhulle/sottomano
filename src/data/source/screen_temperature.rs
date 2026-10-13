use std::time::Duration;

use iced::futures::{Stream, StreamExt, stream};

use crate::data::source::{
    Delivery, PolledSource, Source, command, gamma, present_or_absent, ticked_polls,
};

#[derive(Debug, Clone, PartialEq)]
pub struct SunFollow {
    pub auto: bool,
    pub unit: String,
}

impl Source for SunFollow {
    type Error = command::Error;
    type Key = String;

    const NAME: &'static str = "screen temperature";
}

impl PolledSource for SunFollow {
    const INTERVAL: Duration = Duration::from_secs(10);

    fn polls(unit: String) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(stream::once(command::exists("systemctl")), move || {
            ticked_polls::<Self>(unit.clone()).boxed()
        })
    }

    async fn poll(unit: String) -> Result<Self, Self::Error> {
        let state = command::output(
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
