use std::time::Duration;

use iced::futures::{Stream, stream};

use crate::data::{
    source::{
        DeliveryCadence, ReadingSource, SourceDelivery, command, polled_delivery_stream,
        present_or_absent,
    },
    sun_schedule::ColorMode,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Umbra {
    pub dark: bool,
}

impl ReadingSource for Umbra {
    type Error = command::Error;
    type StreamInputs = ();

    const NAME: &'static str = "umbra";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(30));

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        present_or_absent(stream::once(command::exists("umbra")), move || {
            polled_delivery_stream(inputs, Self::poll)
        })
    }
}

impl Umbra {
    async fn poll((): ()) -> Result<Self, command::Error> {
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
