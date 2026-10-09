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

pub fn either_of(matches: impl IntoIterator<Item = String>) -> impl Iterator<Item = String> {
    matches
        .into_iter()
        .enumerate()
        .flat_map(|(index, matcher)| {
            (index > 0)
                .then(|| String::from("+"))
                .into_iter()
                .chain([matcher])
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JournalPriority {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Info,
    Debug,
}

impl JournalPriority {
    const MOST_URGENT_FIRST: [Self; 8] = [
        JournalPriority::Emergency,
        JournalPriority::Alert,
        JournalPriority::Critical,
        JournalPriority::Error,
        JournalPriority::Warning,
        JournalPriority::Notice,
        JournalPriority::Info,
        JournalPriority::Debug,
    ];

    pub fn of_record(record: &JournalRecord) -> Option<Self> {
        let number: usize = field(record, "PRIORITY")?.parse().ok()?;
        Self::MOST_URGENT_FIRST.get(number).copied()
    }

    pub fn severity(self) -> Severity {
        match self {
            JournalPriority::Emergency | JournalPriority::Alert | JournalPriority::Critical => {
                Severity::Critical
            }
            JournalPriority::Error | JournalPriority::Warning => Severity::Warning,
            JournalPriority::Notice => Severity::Normal,
            JournalPriority::Info | JournalPriority::Debug => Severity::Low,
        }
    }

    pub fn match_this_urgent_or_more(self, journal: &mut Journal) -> io::Result<()> {
        for (number, priority) in Self::MOST_URGENT_FIRST.into_iter().enumerate() {
            if priority > self {
                break;
            }
            journal.match_add("PRIORITY", number.to_string())?;
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

    #[test]
    fn severity_follows_the_syslog_level_names() {
        let severity_of = |priority: &str| {
            let record: JournalRecord = [("PRIORITY".to_owned(), priority.to_owned())].into();
            JournalPriority::of_record(&record).map(JournalPriority::severity)
        };
        assert_eq!(
            ["2", "3", "4", "5", "6", "8"].map(severity_of),
            [
                Some(Severity::Critical),
                Some(Severity::Warning),
                Some(Severity::Warning),
                Some(Severity::Normal),
                Some(Severity::Low),
                None
            ]
        );
    }
}
