use std::{
    collections::VecDeque,
    io,
    os::fd::RawFd,
    str::FromStr,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{Stream, TryStreamExt, future::ready, stream};
use serde::{
    Deserializer,
    de::{
        DeserializeOwned, Error as _, IntoDeserializer, Unexpected, Visitor,
        value::{Error as RecordDecodeError, MapDeserializer},
    },
    forward_to_deserialize_any,
};
use systemd::journal::{Journal, JournalRecord, JournalWaitResult, OpenOptions};
use tokio::{
    io::{Interest, unix::AsyncFd},
    runtime,
    sync::mpsc::{self, error::SendError},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::StateChange;

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

type WindowDelivery<E> = Result<Vec<E>, JournalFollowError>;

pub fn journal_windows<E: JournalEvent>() -> impl Stream<Item = WindowDelivery<E>> + Send {
    stream::once(ready(spawn_journal_follower::<E>()))
        .map_ok(ReceiverStream::new)
        .try_flatten()
}

fn spawn_journal_follower<E: JournalEvent>()
-> Result<mpsc::Receiver<WindowDelivery<E>>, JournalFollowError> {
    let (window_sender, window_receiver) = mpsc::channel(16);
    thread::Builder::new()
        .name(format!("journal:{}", E::NAME))
        .spawn(move || {
            let follower_span = tracing::debug_span!("journal_follower", event = E::NAME);
            let _entered_follower = follower_span.enter();
            let Err(follow_failure) = follow_journal::<E>(&window_sender) else {
                return;
            };
            if let Err(SendError(_undelivered_failure)) =
                window_sender.blocking_send(Err(follow_failure))
            {
                tracing::debug!(
                    event = E::NAME,
                    "journal follower failed after its consumer left"
                );
            }
        })
        .map_err(|spawn_failure| JournalFollowError::OpenFailed(spawn_failure.kind()))?;
    Ok(window_receiver)
}

fn follow_journal<E: JournalEvent>(
    window_sender: &mpsc::Sender<WindowDelivery<E>>,
) -> Result<(), JournalFollowError> {
    let follower_runtime = runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|runtime_failure| JournalFollowError::OpenFailed(runtime_failure.kind()))?;
    follower_runtime.block_on(follow_until_consumer_leaves::<E>(window_sender))
}

async fn follow_until_consumer_leaves<E: JournalEvent>(
    window_sender: &mpsc::Sender<WindowDelivery<E>>,
) -> Result<(), JournalFollowError> {
    let mut journal = tracing::debug_span!("journal_open")
        .in_scope(WindowedJournal::<E>::open)
        .map_err(|open_failure| JournalFollowError::OpenFailed(open_failure.kind()))?;
    loop {
        if let Err(SendError(_undelivered_window)) = window_sender.send(Ok(journal.events())).await
        {
            return Ok(());
        }
        tokio::select! {
            biased;
            () = window_sender.closed() => return Ok(()),
            window_change = journal.window_change() => window_change
                .map_err(|wait_failure| JournalFollowError::FollowingStopped(wait_failure.kind()))?,
        }
    }
}

struct WindowedJournal<E> {
    journal_readiness: AsyncFd<RawFd>,
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
        let journal_readiness =
            unsafe { AsyncFd::register_with_interest(journal.fd()?, Interest::READABLE) }?;
        let mut windowed = Self {
            journal_readiness,
            journal,
            window: EventWindow::default(),
        };
        windowed.rebuild_from_horizon()?;
        Ok(windowed)
    }

    fn events(&self) -> Vec<E> {
        self.window.events.iter().cloned().collect()
    }

    async fn window_change(&mut self) -> io::Result<()> {
        loop {
            self.journal_readiness.readable().await?.clear_ready();
            let _entered_wakeup = tracing::debug_span!("journal_wakeup").entered();
            let journal_change = match self.journal.process()? {
                JournalWaitResult::Append => self.read_appended(),
                JournalWaitResult::Invalidate => {
                    self.rebuild_from_horizon()?;
                    StateChange::Changed
                }
                JournalWaitResult::Nop => StateChange::Unchanged,
            };
            if journal_change == StateChange::Changed {
                self.window.prune_expired(SystemTime::now());
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

    fn read_appended(&mut self) -> StateChange {
        let mut appended = StateChange::Unchanged;
        while let Ok(Some(record)) = self.journal.next_entry() {
            let Ok(at) = self.journal.timestamp() else {
                continue;
            };
            if let Some(event) = admitted_event(record, at) {
                self.window.events.push_back(event);
                appended = StateChange::Changed;
            }
        }
        appended
    }
}

fn admitted_event<E: JournalEvent>(record: JournalRecord, at: SystemTime) -> Option<E> {
    decode_record(record)
        .inspect_err(|error| tracing::debug!("journal: malformed {} record: {error}", E::NAME))
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
    fn prune_expired(&mut self, now: SystemTime) {
        self.events.retain(|event| event.expires_at() > now);
    }
}

pub fn unexpired_events<E: JournalEvent>(
    events: &[E],
    now: SystemTime,
) -> impl Iterator<Item = &E> {
    events.iter().filter(move |event| event.expires_at() > now)
}

fn horizon_cutoff_usec() -> Option<u64> {
    let cutoff = SystemTime::now().checked_sub(EVENT_HORIZON)?;
    let since_epoch = cutoff
        .duration_since(UNIX_EPOCH)
        .inspect_err(|clock_failure| {
            tracing::warn!("journal: horizon before the unix epoch: {clock_failure}");
        })
        .ok()?;
    u64::try_from(since_epoch.as_micros())
        .inspect_err(|range_failure| {
            tracing::warn!("journal: horizon {since_epoch:?} out of range: {range_failure}");
        })
        .ok()
}

pub fn decode_record<Record: DeserializeOwned>(
    record: JournalRecord,
) -> Result<Record, RecordDecodeError> {
    Record::deserialize(MapDeserializer::<_, RecordDecodeError>::new(
        record
            .into_iter()
            .map(|(name, value)| (name, JournalFieldValue { field_text: value })),
    ))
}

struct JournalFieldValue {
    field_text: String,
}

impl JournalFieldValue {
    fn parsed<Number: FromStr>(self) -> Result<Number, RecordDecodeError> {
        self.field_text.parse().map_err(|_| {
            RecordDecodeError::invalid_value(Unexpected::Str(&self.field_text), &"a number")
        })
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
        visitor.visit_string(self.field_text)
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
        visitor.visit_enum(self.field_text.into_deserializer())
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
pub fn journal_record<'fields>(
    fields: impl IntoIterator<Item = (&'fields str, &'fields str)>,
) -> JournalRecord {
    fields
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Stamped {
        stamped_at: SystemTime,
    }

    impl JournalEvent for Stamped {
        type Record = ();

        const NAME: &'static str = "stamped";
        const OPENS_USER_JOURNALS: bool = false;

        fn add_matches(_journal: &mut Journal) -> io::Result<()> {
            Ok(())
        }

        fn admit((): (), at: SystemTime) -> Option<Self> {
            Some(Self { stamped_at: at })
        }

        fn at(&self) -> SystemTime {
            self.stamped_at
        }
    }

    #[test]
    fn appending_prunes_events_past_the_horizon_and_the_display_filter_hides_the_rest() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let minutes = |count: u64| start + Duration::from_secs(count * 60);
        let stamped_at_minutes = |at: u64| Stamped {
            stamped_at: minutes(at),
        };
        let mut window = EventWindow {
            events: [0, 10, 30].map(stamped_at_minutes).into(),
        };
        window.prune_expired(minutes(65));
        assert_eq!(
            window.events,
            [stamped_at_minutes(10), stamped_at_minutes(30)]
        );
        let held_events: Vec<Stamped> = window.events.into();
        assert_eq!(
            unexpired_events(&held_events, minutes(75)).collect::<Vec<_>>(),
            [&stamped_at_minutes(30)]
        );
    }
}
