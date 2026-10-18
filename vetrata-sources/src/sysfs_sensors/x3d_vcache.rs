use std::{fmt, fs, path::PathBuf, str::FromStr, time::Duration};

use futures::{Stream, stream};

use super::{Error, list_dir, read_trimmed};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll, polled_delivery_stream,
    stream_operators::StreamOperators,
};

const X3D_VCACHE_DRIVER: &str = "/sys/bus/platform/drivers/amd_x3d_vcache";
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

    fn mode_file() -> Option<PathBuf> {
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

    #[must_use]
    pub fn write(self) -> bool {
        Self::mode_file().is_some_and(|mode_file| {
            fs::write(&mode_file, self.stored_name())
                .inspect_err(|failure| tracing::warn!("{}: {failure}", mode_file.display()))
                .is_ok()
        })
    }
}

impl ReadingSource for X3dCachePreference {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "x3d-cache-preference";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let exposed = async { Self::mode_file().is_some() };
        stream::once(exposed).while_held(
            move || polled_delivery_stream(inputs, Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl X3dCachePreference {
    async fn poll(_key: ()) -> Result<Self, Error> {
        blocking_poll(Self::read).await
    }
}
