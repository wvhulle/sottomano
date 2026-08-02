use std::time::{Duration, Instant};

use crate::{
    config::disk::NotificationConfig,
    data::notifications::model::{ExpireTimeout, Urgency},
};

#[derive(Debug, Clone, Copy)]
pub enum Countdown {
    Running { deadline: Instant },
    Paused { remaining: Duration },
}

impl Countdown {
    pub fn started(remaining: Duration, now: Instant) -> Self {
        Self::Running {
            deadline: now + remaining,
        }
    }

    pub fn paused(self, now: Instant) -> Self {
        match self {
            Self::Running { deadline } => Self::Paused {
                remaining: deadline.saturating_duration_since(now),
            },
            Self::Paused { .. } => self,
        }
    }

    pub fn resumed(self, now: Instant) -> Self {
        match self {
            Self::Paused { remaining } => Self::started(remaining, now),
            Self::Running { .. } => self,
        }
    }

    pub fn has_elapsed(self, now: Instant) -> bool {
        match self {
            Self::Running { deadline } => deadline <= now,
            Self::Paused { .. } => false,
        }
    }

    pub fn is_running(self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

pub fn lifetime(
    expire_timeout: ExpireTimeout,
    urgency: Urgency,
    config: &NotificationConfig,
) -> Option<Duration> {
    match expire_timeout {
        ExpireTimeout::Never => None,
        ExpireTimeout::After(requested) => Some(requested),
        ExpireTimeout::ServerDefault => {
            let configured_millis = match urgency {
                Urgency::Low => config.low_timeout_ms,
                Urgency::Normal => config.normal_timeout_ms,
                Urgency::Critical => config.critical_timeout_ms,
            };
            (configured_millis > 0).then(|| Duration::from_millis(configured_millis))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> NotificationConfig {
        NotificationConfig {
            low_timeout_ms: 8_000,
            normal_timeout_ms: 12_000,
            critical_timeout_ms: 0,
            ..NotificationConfig::default()
        }
    }

    #[test]
    fn explicit_zero_never_expires() {
        for urgency in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
            assert_eq!(lifetime(ExpireTimeout::Never, urgency, &config()), None);
        }
    }

    #[test]
    fn explicit_timeout_wins_over_urgency_default() {
        let requested = Duration::from_millis(4_000);
        for urgency in [Urgency::Low, Urgency::Normal, Urgency::Critical] {
            assert_eq!(
                lifetime(ExpireTimeout::After(requested), urgency, &config()),
                Some(requested)
            );
        }
    }

    #[test]
    fn server_default_follows_urgency() {
        let config = config();
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Urgency::Low, &config),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Urgency::Normal, &config),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Urgency::Critical, &config),
            None
        );
    }

    #[test]
    fn negative_timeouts_all_mean_server_default() {
        assert_eq!(ExpireTimeout::from_millis(-1), ExpireTimeout::ServerDefault);
        assert_eq!(
            ExpireTimeout::from_millis(i32::MIN),
            ExpireTimeout::ServerDefault
        );
        assert_eq!(ExpireTimeout::from_millis(0), ExpireTimeout::Never);
        assert_eq!(
            ExpireTimeout::from_millis(2_500),
            ExpireTimeout::After(Duration::from_millis(2_500))
        );
    }

    #[test]
    fn pausing_then_resuming_preserves_remaining_time() {
        let start = Instant::now();
        let countdown = Countdown::started(Duration::from_secs(10), start);
        let paused = countdown.paused(start + Duration::from_secs(4));
        let Countdown::Paused { remaining } = paused else {
            panic!("pausing a running countdown must yield Paused");
        };
        assert_eq!(remaining, Duration::from_secs(6));

        let resumed = paused.resumed(start + Duration::from_secs(100));
        assert!(!resumed.has_elapsed(start + Duration::from_secs(105)));
        assert!(resumed.has_elapsed(start + Duration::from_secs(106)));
    }

    #[test]
    fn pausing_after_the_deadline_saturates() {
        let start = Instant::now();
        let countdown = Countdown::started(Duration::from_secs(1), start);
        let Countdown::Paused { remaining } = countdown.paused(start + Duration::from_secs(30))
        else {
            panic!("pausing a running countdown must yield Paused");
        };
        assert_eq!(remaining, Duration::ZERO);
    }

    #[test]
    fn pausing_twice_keeps_the_first_remaining_time() {
        let start = Instant::now();
        let paused = Countdown::started(Duration::from_secs(10), start)
            .paused(start + Duration::from_secs(3));
        let Countdown::Paused { remaining } = paused.paused(start + Duration::from_secs(9)) else {
            panic!("pausing twice must stay Paused");
        };
        assert_eq!(remaining, Duration::from_secs(7));
    }

    #[test]
    fn resuming_a_running_countdown_leaves_the_deadline_alone() {
        let start = Instant::now();
        let running = Countdown::started(Duration::from_secs(10), start);
        let resumed = running.resumed(start + Duration::from_secs(3));
        assert!(!resumed.has_elapsed(start + Duration::from_secs(9)));
        assert!(resumed.has_elapsed(start + Duration::from_secs(10)));
    }

    #[test]
    fn paused_countdowns_never_elapse() {
        let start = Instant::now();
        let paused = Countdown::started(Duration::from_secs(1), start).paused(start);
        assert!(!paused.has_elapsed(start + Duration::from_secs(600)));
        assert!(!paused.is_running());
    }
}
