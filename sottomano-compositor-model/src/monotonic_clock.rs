use std::time::Duration;

use rustix::time::{ClockId, clock_gettime};

#[must_use]
pub fn monotonic_since_boot() -> Duration {
    let spec = clock_gettime(ClockId::Monotonic);
    let secs = u64::try_from(spec.tv_sec).expect("monotonic clock seconds are never negative");
    let nanos =
        u32::try_from(spec.tv_nsec).expect("monotonic clock nanoseconds are below one second");
    Duration::new(secs, nanos)
}
