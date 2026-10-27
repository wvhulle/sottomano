use std::{
    future::ready,
    io,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt};
use serde::Deserialize;
use systemd::journal::Journal;

use crate::{
    system_protocol_clients::journal_follow::{JournalEvent, journal_windows},
    systemd_units_journal::journal_record_fields::{JournalPriority, JournalTransport},
};

const DEVICE_ERROR_HORIZON: Duration = Duration::from_mins(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKernelError {
    pub at: SystemTime,
    pub kernel_device: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceKernelRecord {
    #[serde(rename = "MESSAGE")]
    message: String,
    #[serde(rename = "_KERNEL_DEVICE")]
    kernel_device: String,
}

fn message_without_device_prefix(message: &str, kernel_device: &str) -> String {
    kernel_device
        .split_once(':')
        .and_then(|(_, device_name)| message.split_once(&format!("{device_name}: ")))
        .map_or(message, |(_, detail)| detail)
        .to_owned()
}

impl JournalEvent for DeviceKernelError {
    type Record = DeviceKernelRecord;

    const NAME: &'static str = "device kernel errors";
    const OPENS_USER_JOURNALS: bool = false;

    fn add_matches(journal: &mut Journal) -> io::Result<()> {
        JournalTransport::Kernel.add_match(journal)?;
        JournalPriority::Warning.match_this_urgent_or_more(journal)
    }

    fn admit(record: DeviceKernelRecord, at: SystemTime) -> Option<Self> {
        Some(DeviceKernelError {
            at,
            message: message_without_device_prefix(&record.message, &record.kernel_device),
            kernel_device: record.kernel_device,
        })
    }

    fn at(&self) -> SystemTime {
        self.at
    }

    fn expires_at(&self) -> SystemTime {
        self.at + DEVICE_ERROR_HORIZON
    }
}

pub fn device_kernel_error_windows() -> impl Stream<Item = Vec<DeviceKernelError>> + Send {
    journal_windows::<DeviceKernelError>().filter_map(|window| {
        ready(
            window
                .inspect_err(|e| tracing::warn!("device kernel errors: {e}"))
                .ok(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_driver_and_device_prefix_is_dropped_from_the_message() {
        assert_eq!(
            message_without_device_prefix(
                "playstation 0005:054C:0CE6.0011: DualSense input CRC's check failed",
                "+hid:0005:054C:0CE6.0011",
            ),
            "DualSense input CRC's check failed"
        );
    }

    #[test]
    fn a_message_without_the_device_name_stays_whole() {
        assert_eq!(
            message_without_device_prefix("link reset", "+usb:1-4"),
            "link reset"
        );
    }
}
