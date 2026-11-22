use iced::time::{Duration, Instant};
use sottomano_desktop_sources::desktop_notifications::notification_data::ExpireTimeout;
use sottomano_toml_config::{notification_toasts::ToastLifetimeConfig, severity::Severity};

#[derive(Debug, Clone, Copy)]
pub enum Countdown {
    Running { deadline: Instant },
    Paused { remaining: Duration },
    Unlimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLifetime {
    Limited(Duration),
    Unlimited,
}

impl Countdown {
    #[must_use]
    pub fn started(remaining: Duration, now: Instant) -> Self {
        Self::Running {
            deadline: now + remaining,
        }
    }

    #[must_use]
    pub fn counting(toast_lifetime: ToastLifetime, now: Instant) -> Self {
        match toast_lifetime {
            ToastLifetime::Limited(remaining) => Self::started(remaining, now),
            ToastLifetime::Unlimited => Self::Unlimited,
        }
    }

    #[must_use]
    #[expect(
        clippy::disallowed_methods,
        reason = "a toast paused after its deadline but before the expiry tick has no time left"
    )]
    pub fn paused(self, now: Instant) -> Self {
        match self {
            Self::Running { deadline } => Self::Paused {
                remaining: deadline.saturating_duration_since(now),
            },
            Self::Paused { .. } | Self::Unlimited => self,
        }
    }

    #[must_use]
    pub fn resumed(self, now: Instant) -> Self {
        match self {
            Self::Paused { remaining } => Self::started(remaining, now),
            Self::Running { .. } | Self::Unlimited => self,
        }
    }

    #[must_use]
    pub fn expiry_at(self, now: Instant) -> CountdownExpiry {
        match self {
            Self::Running { deadline } if deadline <= now => CountdownExpiry::Elapsed,
            Self::Running { .. } | Self::Paused { .. } | Self::Unlimited => {
                CountdownExpiry::Pending
            }
        }
    }

    #[must_use]
    pub fn deadline(self) -> Option<Instant> {
        match self {
            Self::Running { deadline } => Some(deadline),
            Self::Paused { .. } | Self::Unlimited => None,
        }
    }
}

#[must_use]
pub fn lifetime(
    expire_timeout: ExpireTimeout,
    severity: Severity,
    config: &ToastLifetimeConfig,
) -> ToastLifetime {
    match expire_timeout {
        ExpireTimeout::Never => ToastLifetime::Unlimited,
        ExpireTimeout::After(requested) => ToastLifetime::Limited(requested),
        ExpireTimeout::ServerDefault => match config.default_timeout_for(severity) {
            Some(default_timeout) => ToastLifetime::Limited(default_timeout),
            None => ToastLifetime::Unlimited,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountdownExpiry {
    Elapsed,
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    fn config() -> ToastLifetimeConfig {
        ToastLifetimeConfig {
            low_severity_timeout: Duration::from_secs(8),
            normal_severity_timeout: Duration::from_secs(12),
            critical_severity_timeout: Duration::ZERO,
        }
    }

    #[test]
    fn explicit_zero_never_expires() {
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(
                lifetime(ExpireTimeout::Never, severity, &config()),
                ToastLifetime::Unlimited
            );
        }
    }

    #[test]
    fn explicit_timeout_wins_over_severity_default() {
        let requested = Duration::from_millis(4_000);
        for severity in [Severity::Low, Severity::Normal, Severity::Critical] {
            assert_eq!(
                lifetime(ExpireTimeout::After(requested), severity, &config()),
                ToastLifetime::Limited(requested)
            );
        }
    }

    #[test]
    fn server_default_follows_severity() {
        let config = config();
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Low, &config),
            ToastLifetime::Limited(Duration::from_secs(8))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Normal, &config),
            ToastLifetime::Limited(Duration::from_secs(12))
        );
        assert_eq!(
            lifetime(ExpireTimeout::ServerDefault, Severity::Critical, &config),
            ToastLifetime::Unlimited
        );
    }

    #[test]
    fn pausing_then_resuming_preserves_remaining_time() {
        let start = Clock::real().instant;
        let countdown = Countdown::started(Duration::from_secs(10), start);
        let paused = countdown.paused(start + Duration::from_secs(4));
        let Countdown::Paused { remaining } = paused else {
            panic!("pausing a running countdown must yield Paused");
        };
        assert_eq!(remaining, Duration::from_secs(6));

        let resumed = paused.resumed(start + Duration::from_secs(100));
        assert_eq!(
            resumed.expiry_at(start + Duration::from_secs(105)),
            CountdownExpiry::Pending
        );
        assert_eq!(
            resumed.expiry_at(start + Duration::from_secs(106)),
            CountdownExpiry::Elapsed
        );
    }

    #[test]
    fn paused_countdowns_never_elapse() {
        let start = Clock::real().instant;
        let paused = Countdown::started(Duration::from_secs(1), start).paused(start);
        assert_eq!(
            paused.expiry_at(start + Duration::from_secs(600)),
            CountdownExpiry::Pending
        );
        assert_eq!(paused.deadline(), None);
    }
}
