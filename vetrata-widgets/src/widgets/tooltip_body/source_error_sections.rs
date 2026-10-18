use std::sync::Arc;

use system_tray::error;
use vetrata_sources::{
    Panicked,
    ai::{claude, copilot, openrouter},
    audio::AudioConnectError,
    bus, command, disk, gpu,
    hardware::HostHardwareError,
    http,
    journal::JournalFollowError,
    nix_package_facts,
    previous_shutdown::ShutdownReadError,
    process, sysfs_sensors, tailscale, weather,
};

use crate::widgets::tooltip_body::{KeyValue, Section};

impl From<Panicked> for Section {
    fn from(panicked: Panicked) -> Self {
        Section::error(panicked)
    }
}

impl From<gpu::Error> for Section {
    fn from(error: gpu::Error) -> Self {
        Section::error(error)
    }
}

impl From<disk::Error> for Section {
    fn from(error: disk::Error) -> Self {
        Section::error(error)
    }
}

impl From<tailscale::Error> for Section {
    fn from(error: tailscale::Error) -> Self {
        Section::error(error)
    }
}

impl From<bus::Error> for Section {
    fn from(error: bus::Error) -> Self {
        Section::error(error)
    }
}

impl From<zbus::Error> for Section {
    fn from(error: zbus::Error) -> Self {
        Section::error(error)
    }
}

impl From<nix_package_facts::Error> for Section {
    fn from(error: nix_package_facts::Error) -> Self {
        Section::error(error)
    }
}

impl From<process::Error> for Section {
    fn from(error: process::Error) -> Self {
        Section::error(error)
    }
}

impl From<Arc<error::Error>> for Section {
    fn from(error: Arc<error::Error>) -> Self {
        Section::error(error)
    }
}

impl From<sysfs_sensors::Error> for Section {
    fn from(error: sysfs_sensors::Error) -> Self {
        Section::error(error)
    }
}

impl From<HostHardwareError> for Section {
    fn from(error: HostHardwareError) -> Self {
        Section::error(error)
    }
}

impl From<ShutdownReadError> for Section {
    fn from(error: ShutdownReadError) -> Self {
        Section::error(error)
    }
}

impl From<JournalFollowError> for Section {
    fn from(error: JournalFollowError) -> Self {
        Section::error(error)
    }
}

impl From<AudioConnectError> for Section {
    fn from(error: AudioConnectError) -> Self {
        Section::error(error)
    }
}

impl From<command::Error> for Section {
    fn from(error: command::Error) -> Self {
        Section::error(error)
    }
}

impl From<http::Error> for Section {
    fn from(error: http::Error) -> Self {
        match error {
            http::Error::Status { status, message } => Section::dictionary(
                message
                    .map(|message| KeyValue::new("Error", message))
                    .into_iter()
                    .chain([KeyValue::new("Status", status.to_string())]),
            ),
            other => Section::error(other),
        }
    }
}

impl From<claude::Error> for Section {
    fn from(error: claude::Error) -> Self {
        match error {
            claude::Error::RateLimited(cause) | claude::Error::Fetch(cause) => cause.into(),
            claude::Error::Backoff { remaining, cause } => {
                Section::from(cause).with_entries([KeyValue::new(
                    "Retry in",
                    humantime::format_duration(remaining).to_string(),
                )])
            }
            other => Section::error(other),
        }
    }
}

impl From<weather::Error> for Section {
    fn from(error: weather::Error) -> Self {
        match error {
            weather::Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl From<openrouter::Error> for Section {
    fn from(error: openrouter::Error) -> Self {
        match error {
            openrouter::Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl From<copilot::Error> for Section {
    fn from(error: copilot::Error) -> Self {
        match error {
            copilot::Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
    }
}
