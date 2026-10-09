use std::{
    collections::VecDeque,
    io, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use systemd::journal::{Journal, JournalRecord, JournalWaitResult, OpenOptions};
use tokio::sync::mpsc;

use crate::data::severity::Severity;

pub const EVENT_HORIZON: Duration = Duration::from_secs(3600);

pub trait JournalEvent: Clone + Send + 'static {
    const NAME: &'static str;
    const OPENS_USER_JOURNALS: bool;

    fn add_matches(journal: &mut Journal) -> io::Result<()>;

    fn from_record(record: &JournalRecord, at: SystemTime) -> Option<Self>;

    fn at(&self) -> SystemTime;
}

pub fn journal_events<E: JournalEvent>() -> mpsc::Receiver<Vec<E>> {
    let (tx, rx) = mpsc::channel(16);
    thread::spawn(move || {
        let mut journal = match WindowedJournal::<E>::open() {
            Ok(journal) => journal,
            Err(e) => {
                tracing::warn!("journal: {} open failed: {e}", E::NAME);
                return;
            }
        };
        while tx.blocking_send(journal.events()).is_ok() {
            if let Err(e) = journal.wait_for_window_change() {
                tracing::warn!("journal: {} wait ended: {e}", E::NAME);
                return;
            }
        }
    });
    rx
}

struct WindowedJournal<E> {
    journal: Journal,
    window: EventWindow<E>,
}

impl<E: JournalEvent> WindowedJournal<E> {
    fn open() -> io::Result<Self> {
        let mut journal = OpenOptions::default()
            .system(true)
            .current_user(E::OPENS_USER_JOURNALS)
            .open()?;
        E::add_matches(&mut journal)?;
        let mut windowed = Self {
            journal,
            window: EventWindow::default(),
        };
        windowed.rebuild_from_horizon()?;
        Ok(windowed)
    }

    fn events(&self) -> Vec<E> {
        self.window.events.iter().cloned().collect()
    }

    fn wait_for_window_change(&mut self) -> io::Result<()> {
        loop {
            let timeout = self.window.time_until_next_expiry(SystemTime::now());
            let appended = match self.journal.wait(timeout)? {
                JournalWaitResult::Append => self.read_appended(),
                JournalWaitResult::Invalidate => {
                    self.rebuild_from_horizon()?;
                    true
                }
                JournalWaitResult::Nop => false,
            };
            let expired = self.window.drop_expired(SystemTime::now());
            if appended || expired {
                return Ok(());
            }
        }
    }

    fn rebuild_from_horizon(&mut self) -> io::Result<()> {
        self.window = EventWindow::default();
        let cutoff_usec = horizon_cutoff_usec().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "system clock precedes the event horizon",
            )
        })?;
        self.journal.seek_realtime_usec(cutoff_usec)?;
        self.read_appended();
        Ok(())
    }

    fn read_appended(&mut self) -> bool {
        let mut appended = false;
        while let Ok(Some(record)) = self.journal.next_entry() {
            let event = self
                .journal
                .timestamp()
                .ok()
                .and_then(|at| E::from_record(&record, at));
            if let Some(event) = event {
                self.window.events.push_back(event);
                appended = true;
            }
        }
        appended
    }
}

struct EventWindow<E> {
    events: VecDeque<E>,
}

impl<E> Default for EventWindow<E> {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }
}

impl<E: JournalEvent> EventWindow<E> {
    fn expiry_of(event: &E) -> SystemTime {
        event.at() + EVENT_HORIZON
    }

    fn drop_expired(&mut self, now: SystemTime) -> bool {
        let before = self.events.len();
        while self
            .events
            .front()
            .is_some_and(|oldest| Self::expiry_of(oldest) <= now)
        {
            self.events.pop_front();
        }
        self.events.len() != before
    }

    fn time_until_next_expiry(&self, now: SystemTime) -> Option<Duration> {
        let oldest = self.events.front()?;
        Some(
            Self::expiry_of(oldest)
                .duration_since(now)
                .unwrap_or(Duration::ZERO),
        )
    }
}

fn horizon_cutoff_usec() -> Option<u64> {
    let cutoff = SystemTime::now().checked_sub(EVENT_HORIZON)?;
    u64::try_from(cutoff.duration_since(UNIX_EPOCH).ok()?.as_micros()).ok()
}

pub fn field<'a>(record: &'a JournalRecord, name: &str) -> Option<&'a str> {
    record.get(name).map(String::as_str)
}

impl Severity {
    fn least_urgent_journal_priority(self) -> Option<u8> {
        match self {
            Severity::Critical => Some(3),
            Severity::Warning => Some(4),
            Severity::Low | Severity::Normal => None,
        }
    }

    pub fn of_journal_record(record: &JournalRecord) -> Option<Self> {
        let priority: u8 = field(record, "PRIORITY")?.parse().ok()?;
        [Severity::Critical, Severity::Warning]
            .into_iter()
            .find(|severity| {
                severity
                    .least_urgent_journal_priority()
                    .is_some_and(|least_urgent| priority <= least_urgent)
            })
    }

    pub fn match_journal_priorities(self, journal: &mut Journal) -> io::Result<()> {
        let least_urgent = self.least_urgent_journal_priority().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "severity has no journal priority",
            )
        })?;
        for priority in 0..=least_urgent {
            journal.match_add("PRIORITY", priority.to_string())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwningUnit {
    System(String),
    User(String),
}

impl OwningUnit {
    pub fn from_fields(
        record: &JournalRecord,
        system_field: &str,
        user_field: &str,
    ) -> Option<Self> {
        field(record, user_field)
            .map(|name| OwningUnit::User(name.to_owned()))
            .or_else(|| field(record, system_field).map(|name| OwningUnit::System(name.to_owned())))
    }

    pub fn name(&self) -> &str {
        match self {
            OwningUnit::System(name) | OwningUnit::User(name) => name,
        }
    }

    pub fn logged_by_match(&self) -> String {
        match self {
            OwningUnit::System(name) => format!("_SYSTEMD_UNIT={name}"),
            OwningUnit::User(name) => format!("_SYSTEMD_USER_UNIT={name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Stamped(SystemTime);

    impl JournalEvent for Stamped {
        const NAME: &'static str = "stamped";
        const OPENS_USER_JOURNALS: bool = false;

        fn add_matches(_journal: &mut Journal) -> io::Result<()> {
            Ok(())
        }

        fn from_record(_record: &JournalRecord, at: SystemTime) -> Option<Self> {
            Some(Self(at))
        }

        fn at(&self) -> SystemTime {
            self.0
        }
    }

    #[test]
    fn expired_events_leave_the_window_and_the_next_expiry_follows_the_oldest_survivor() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let minutes = |count: u64| start + Duration::from_secs(count * 60);
        let mut window = EventWindow {
            events: [0, 10, 30].map(|at| Stamped(minutes(at))).into(),
        };
        let now = minutes(65);
        assert!(window.drop_expired(now));
        assert_eq!(window.events, [Stamped(minutes(10)), Stamped(minutes(30))]);
        assert_eq!(
            window.time_until_next_expiry(now),
            Some(Duration::from_mins(5))
        );
        assert!(!window.drop_expired(now));
    }
}
