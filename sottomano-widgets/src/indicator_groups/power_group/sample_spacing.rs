use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub(super) enum SampleSpacing {
    Interrupted,
    LastSampledAt(Instant),
}

impl SampleSpacing {
    pub(super) fn sampled_now(&mut self, now: Instant) -> Duration {
        let since_previous = match *self {
            SampleSpacing::Interrupted => Duration::ZERO,
            SampleSpacing::LastSampledAt(previous) => now - previous,
        };
        *self = SampleSpacing::LastSampledAt(now);
        since_previous
    }
}
