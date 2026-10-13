use std::time::Duration;

use iced::futures::{Stream, StreamExt, stream};

use crate::data::{
    source::{Delivery, PolledSource, Source, command, present_or_absent, ticked_polls},
    sun_schedule::ColorMode,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Umbra {
    pub dark: bool,
}

impl Source for Umbra {
    type Error = command::Error;
    type Key = ();

    const NAME: &'static str = "umbra";
}

impl PolledSource for Umbra {
    const INTERVAL: Duration = Duration::from_secs(30);

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(stream::once(command::exists("umbra")), move || {
            ticked_polls::<Self>(key).boxed()
        })
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let mode = command::output("umbra", &["get"]).await?;
        Ok(Umbra {
            dark: mode.trim() == "dark",
        })
    }
}

pub fn toggle_mode() -> bool {
    command::blocking::run("umbra", &["toggle"])
}

pub fn set_mode(mode: ColorMode) -> bool {
    command::blocking::run("umbra", &["set", mode.umbra_name()])
}
