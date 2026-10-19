use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Local, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub instant: Instant,
    pub wall: SystemTime,
}

impl Clock {
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn real() -> Self {
        Self {
            instant: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    #[must_use]
    pub fn utc(self) -> DateTime<Utc> {
        DateTime::<Utc>::from(self.wall)
    }

    #[must_use]
    pub fn local(self) -> DateTime<Local> {
        self.utc().with_timezone(&Local)
    }

    #[must_use]
    pub fn advanced(self, by: Duration) -> Self {
        Self {
            instant: self.instant + by,
            wall: self.wall + by,
        }
    }
}
