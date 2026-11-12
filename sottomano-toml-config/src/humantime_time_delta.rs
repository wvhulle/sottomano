use std::time::Duration;

use chrono::TimeDelta;
use serde::{Deserializer, Serializer, de::Error as DeserializeError};

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TimeDelta, D::Error> {
    let written_length: Duration = humantime_serde::deserialize(deserializer)?;
    TimeDelta::from_std(written_length).map_err(|out_of_range| {
        D::Error::custom(format!(
            "{} is longer than the calendar can represent: {out_of_range}",
            humantime::format_duration(written_length)
        ))
    })
}

pub fn serialize<S: Serializer>(length: &TimeDelta, serializer: S) -> Result<S::Ok, S::Error> {
    let written_length = length
        .to_std()
        .expect("configured lengths are read from std durations, so they are never negative");
    humantime_serde::serialize(&written_length, serializer)
}
