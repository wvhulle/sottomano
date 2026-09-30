use std::time::{Duration, Instant};

use crate::{
    config::toasts::ToastLifetimeConfig,
    data::{notifications::model::ExpireTimeout, severity::Severity},
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
    severity: Severity,
    config: &ToastLifetimeConfig,
) -> Option<Duration> {
    match expire_timeout {
        ExpireTimeout::Never => None,
        ExpireTimeout::After(requested) => Some(requested),
        ExpireTimeout::ServerDefault => config.default_timeout_for(severity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ToastLifetimeConfig {
        ToastLifetimeConfig {
            low_severity_timeout_ms: 8_000,
            normal_severity_timeout_ms: 12_000,
            critical_severity_timeout_ms: 0,
        }
    }

    #[test]
    fn explicit_zero_never_expires() {
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(lifetime(ExpireTimeout::Never, severity, &config()), None);
        }
    }

    #[test]
    fn explicit_timeout_wins_over_severity_default() {
        let requested = Duration::from_millis(4_000);
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(
                lifetime(ExpireTimeout::After(requested), severity, &config()),
                Some(requested)
            );
        }
    }

    #[test]
    fn server_default_follows_severity() {
        let config = config();
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Low, &config),
            Some(Duration::from_secs(8))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Normal, &config),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Critical, &config),
            None
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
    fn paused_countdowns_never_elapse() {
        let start = Instant::now();
        let paused = Countdown::started(Duration::from_secs(1), start).paused(start);
        assert!(!paused.has_elapsed(start + Duration::from_secs(600)));
        assert!(!paused.is_running());
    }
}
