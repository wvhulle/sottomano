use std::{
    collections::VecDeque,
    io,
    str::FromStr,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::Stream;
use serde::{
    Deserializer,
    de::{
        DeserializeOwned, Error as _, IntoDeserializer, Unexpected, Visitor,
        value::{Error as RecordDecodeError, MapDeserializer},
    },
    forward_to_deserialize_any,
};
use systemd::journal::{Journal, JournalRecord, JournalWaitResult, OpenOptions};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub const EVENT_HORIZON: Duration = Duration::from_secs(3600);

pub trait JournalEvent: Clone + Send + 'static {
    type Record: DeserializeOwned;

    const NAME: &'static str;
    const OPENS_USER_JOURNALS: bool;

    fn add_matches(journal: &mut Journal) -> io::Result<()>;

    fn admit(record: Self::Record, at: SystemTime) -> Option<Self>;

    fn at(&self) -> SystemTime;

    fn expires_at(&self) -> SystemTime {
        self.at() + EVENT_HORIZON
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JournalFollowError {
    #[error("cannot open the journal: {0}")]
    OpenFailed(io::ErrorKind),
    #[error("stopped following the journal: {0}")]
    FollowingStopped(io::ErrorKind),
}

pub fn journal_windows<E: JournalEvent>()
-> impl Stream<Item = Result<Vec<E>, JournalFollowError>> + Send {
    ReceiverStream::new(journal_events::<E>())
}

fn journal_events<E: JournalEvent>() -> mpsc::Receiver<Result<Vec<E>, JournalFollowError>> {
    let (tx, rx) = mpsc::channel(16);
    thread::spawn(move || {
        let follower_span = tracing::debug_span!("journal_follower", event = E::NAME);
        let _entered_follower = follower_span.enter();
        let mut journal =
            match tracing::debug_span!("journal_open").in_scope(WindowedJournal::<E>::open) {
                Ok(journal) => journal,
                Err(e) => {
                    let _ = tx.blocking_send(Err(JournalFollowError::OpenFailed(e.kind())));
                    return;
                }
            };
        while tx.blocking_send(Ok(journal.events())).is_ok() {
            if let Err(e) = journal.wait_for_window_change() {
                let _ = tx.blocking_send(Err(JournalFollowError::FollowingStopped(e.kind())));
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
            let wait_result = self.journal.wait(timeout)?;
            let _entered_wakeup = tracing::debug_span!("journal_wakeup").entered();
            let appended = match wait_result {
                JournalWaitResult::Append => self.read_appended(),
                JournalWaitResult::Invalidate => {
                    self.rebuild_from_horizon()?;
                    WindowChange::Changed
                }
                JournalWaitResult::Nop => WindowChange::Unchanged,
            };
            let expired = self.window.drop_expired(SystemTime::now());
            if appended == WindowChange::Changed || expired == WindowChange::Changed {
                return Ok(());
            }
        }
    }

    fn rebuild_from_horizon(&mut self) -> io::Result<()> {
        let _entered_rebuild = tracing::debug_span!("journal_rebuild_from_horizon").entered();
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

    fn read_appended(&mut self) -> WindowChange {
        let mut appended = WindowChange::Unchanged;
        while let Ok(Some(record)) = self.journal.next_entry() {
            let Ok(at) = self.journal.timestamp() else {
                continue;
            };
            if let Some(event) = admitted_event(record, at) {
                self.window.events.push_back(event);
                appended = WindowChange::Changed;
            }
        }
        appended
    }
}

fn admitted_event<E: JournalEvent>(record: JournalRecord, at: SystemTime) -> Option<E> {
    decode_record(record)
        .inspect_err(|e| tracing::debug!("journal: malformed {} record: {e}", E::NAME))
        .ok()
        .and_then(|record| E::admit(record, at))
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
    fn drop_expired(&mut self, now: SystemTime) -> WindowChange {
        let before = self.events.len();
        while self
            .events
            .front()
            .is_some_and(|oldest| oldest.expires_at() <= now)
        {
            self.events.pop_front();
        }
        if self.events.len() == before {
            WindowChange::Unchanged
        } else {
            WindowChange::Changed
        }
    }

    fn time_until_next_expiry(&self, now: SystemTime) -> Option<Duration> {
        let oldest = self.events.front()?;
        Some(
            oldest
                .expires_at()
                .duration_since(now)
                .unwrap_or(Duration::ZERO),
        )
    }
}

fn horizon_cutoff_usec() -> Option<u64> {
    let cutoff = SystemTime::now().checked_sub(EVENT_HORIZON)?;
    u64::try_from(cutoff.duration_since(UNIX_EPOCH).ok()?.as_micros()).ok()
}

pub fn decode_record<Record: DeserializeOwned>(
    record: JournalRecord,
) -> Result<Record, RecordDecodeError> {
    Record::deserialize(MapDeserializer::<_, RecordDecodeError>::new(
        record
            .into_iter()
            .map(|(name, value)| (name, JournalFieldValue(value))),
    ))
}

struct JournalFieldValue(String);

impl JournalFieldValue {
    fn parsed<Number: FromStr>(self) -> Result<Number, RecordDecodeError> {
        self.0
            .parse()
            .map_err(|_| RecordDecodeError::invalid_value(Unexpected::Str(&self.0), &"a number"))
    }
}

impl IntoDeserializer<'_, RecordDecodeError> for JournalFieldValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

macro_rules! deserialize_parsed_numbers {
    ($($deserialize:ident => $visit:ident),* $(,)?) => {
        $(
            fn $deserialize<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                visitor.$visit(self.parsed()?)
            }
        )*
    };
}

impl<'de> Deserializer<'de> for JournalFieldValue {
    type Error = RecordDecodeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.0)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_some(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_enum(self.0.into_deserializer())
    }

    deserialize_parsed_numbers! {
        deserialize_u8 => visit_u8,
        deserialize_u16 => visit_u16,
        deserialize_u32 => visit_u32,
        deserialize_u64 => visit_u64,
        deserialize_i8 => visit_i8,
        deserialize_i16 => visit_i16,
        deserialize_i32 => visit_i32,
        deserialize_i64 => visit_i64,
    }

    forward_to_deserialize_any! {
        bool i128 u128 f32 f64 char str string bytes byte_buf unit unit_struct
        newtype_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

#[cfg(test)]
pub fn journal_record<'a>(fields: impl IntoIterator<Item = (&'a str, &'a str)>) -> JournalRecord {
    fields
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowChange {
    Changed,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Stamped(SystemTime);

    impl JournalEvent for Stamped {
        type Record = ();

        const NAME: &'static str = "stamped";
        const OPENS_USER_JOURNALS: bool = false;

        fn add_matches(_journal: &mut Journal) -> io::Result<()> {
            Ok(())
        }

        fn admit((): (), at: SystemTime) -> Option<Self> {
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
        assert_eq!(window.drop_expired(now), WindowChange::Changed);
        assert_eq!(window.events, [Stamped(minutes(10)), Stamped(minutes(30))]);
        assert_eq!(
            window.time_until_next_expiry(now),
            Some(Duration::from_mins(5))
        );
        assert_eq!(window.drop_expired(now), WindowChange::Unchanged);
    }
}
