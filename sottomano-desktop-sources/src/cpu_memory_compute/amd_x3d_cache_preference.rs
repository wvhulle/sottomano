use std::{fmt, path::PathBuf, str::FromStr};

use futures::{Stream, StreamExt, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::{Availability, AvailabilityStreams},
    blocking_background_work::blocking_poll,
    system_protocol_clients::sysfs_attribute_reads::{Error, list_dir, read_trimmed},
};

pub const X3D_VCACHE_DRIVER: &str = "/sys/bus/platform/drivers/amd_x3d_vcache";
const MODE_FILE: &str = "amd_x3d_mode";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X3dCachePreference {
    Cache,
    Frequency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownX3dMode(pub String);

impl fmt::Display for UnknownX3dMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown X3D mode {}", self.0)
    }
}

impl FromStr for X3dCachePreference {
    type Err = UnknownX3dMode;

    fn from_str(mode: &str) -> Result<Self, UnknownX3dMode> {
        match mode {
            "cache" => Ok(X3dCachePreference::Cache),
            "frequency" => Ok(X3dCachePreference::Frequency),
            _ => Err(UnknownX3dMode(mode.to_owned())),
        }
    }
}

impl X3dCachePreference {
    #[must_use]
    pub const fn stored_name(self) -> &'static str {
        match self {
            X3dCachePreference::Cache => "cache",
            X3dCachePreference::Frequency => "frequency",
        }
    }

    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            X3dCachePreference::Cache => X3dCachePreference::Frequency,
            X3dCachePreference::Frequency => X3dCachePreference::Cache,
        }
    }

    #[must_use]
    pub fn mode_file() -> Option<PathBuf> {
        list_dir(X3D_VCACHE_DRIVER)
            .into_iter()
            .map(|device| device.join(MODE_FILE))
            .find(|mode| mode.exists())
    }

    fn read() -> Result<Self, Error> {
        let mode_file =
            Self::mode_file().ok_or_else(|| Error::Unreadable(X3D_VCACHE_DRIVER.to_owned()))?;
        let mode = read_trimmed(&mode_file)
            .ok_or_else(|| Error::Unreadable(mode_file.display().to_string()))?;
        mode.parse().map_err(|_| Error::Unparsable(mode))
    }
}

impl SnapshotSource for X3dCachePreference {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "X3D scheduling preference";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let exposed = async {
            match Self::mode_file() {
                Some(_) => Availability::Available,
                None => Availability::Unavailable,
            }
        };
        stream::once(exposed).while_available(
            || stream::once(blocking_poll(Self::read)).map(SnapshotDelivery::single_read),
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}
