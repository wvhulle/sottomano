use std::{
    collections::VecDeque,
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    str::FromStr,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::{Stream, StreamExt, future::ready, stream};
use serde::{
    Deserialize, Deserializer,
    de::{
        DeserializeOwned, Error as _, IntoDeserializer, Unexpected, Visitor,
        value::{Error as RecordDecodeError, MapDeserializer},
    },
    forward_to_deserialize_any,
};
use sottomano_config::severity::Severity;
use systemd::journal::{Journal, JournalRecord, JournalWaitResult, OpenOptions};
use tokio::sync::mpsc::{self, error::TrySendError};
use tokio_stream::wrappers::ReceiverStream;

use crate::systemd_manager::ScopedSystemdUnit;

pub const EVENT_HORIZON: Duration = Duration::from_secs(3600);

pub trait JournalEvent: Clone + Send + 'static {
    type Record: DeserializeOwned;

    const NAME: &'static str;
    const OPENS_USER_JOURNALS: bool;

    fn add_matches(journal: &mut Journal) -> io::Result<()>;

    fn admit(record: Self::Record, at: SystemTime) -> Option<Self>;

    fn at(&self) -> SystemTime;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalFollowError {
    OpenFailed(io::ErrorKind),
    FollowingStopped(io::ErrorKind),
}

impl Display for JournalFollowError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            JournalFollowError::OpenFailed(kind) => write!(f, "cannot open the journal: {kind}"),
            JournalFollowError::FollowingStopped(kind) => {
                write!(f, "stopped following the journal: {kind}")
            }
        }
    }
}

pub fn journal_windows<E: JournalEvent>()
-> impl Stream<Item = Result<Vec<E>, JournalFollowError>> + Send {
    ReceiverStream::new(journal_events::<E>())
}

fn journal_events<E: JournalEvent>() -> mpsc::Receiver<Result<Vec<E>, JournalFollowError>> {
    let (tx, rx) = mpsc::channel(16);
    thread::spawn(move || {
        let mut journal = match WindowedJournal::<E>::open() {
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

pub fn unit_log_appends(unit: ScopedSystemdUnit) -> impl Stream<Item = ()> + Send {
    let (tx, rx) = mpsc::channel(1);
    thread::spawn(move || {
        if let Err(e) = signal_unit_log_appends(&unit, &tx) {
            tracing::warn!("journal: following {} ended: {e}", unit.unit_name);
        }
    });
    stream::once(ready(())).chain(ReceiverStream::new(rx))
}

fn signal_unit_log_appends(unit: &ScopedSystemdUnit, tx: &mpsc::Sender<()>) -> io::Result<()> {
    let per_user = unit.scope.per_user();
    let mut journal = OpenOptions::default()
        .system(!per_user)
        .current_user(per_user)
        .open()?;
    journal.match_add(
        unit.scope.journal_logged_by_field(),
        unit.unit_name.as_str(),
    )?;
    journal.seek_tail()?;
    journal.previous()?;
    while !tx.is_closed() {
        if matches!(journal.wait(None)?, JournalWaitResult::Nop) {
            continue;
        }
        while journal.next()? > 0 {}
        if let Err(TrySendError::Closed(())) = tx.try_send(()) {
            break;
        }
    }
    Ok(())
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
            let Ok(at) = self.journal.timestamp() else {
                continue;
            };
            if let Some(event) = Self::admitted_event(record, at) {
                self.window.events.push_back(event);
                appended = true;
            }
        }
        appended
    }

    fn admitted_event(record: JournalRecord, at: SystemTime) -> Option<E> {
        decode_record(record)
            .inspect_err(|e| tracing::debug!("journal: malformed {} record: {e}", E::NAME))
            .ok()
            .and_then(|record| E::admit(record, at))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct MessageId(u128);

impl MessageId {
    pub const COREDUMP: Self = Self(0xfc2e_22bc_6ee6_47b6_b907_29ab_34a2_50b1);
    pub const KERNEL_OOM_KILL: Self = Self(0xfe6f_aa94_e777_4663_a0da_5271_7891_d8ef);
    pub const OOMD_KILL: Self = Self(0xd989_611b_15e4_4c9d_bf31_e3c8_1256_e4ed);
    pub const SHUTDOWN: Self = Self(0x9826_8866_d1d5_4a49_9c4e_9892_1d93_bc40);
    pub const UNIT_STOPPING: Self = Self(0xde5b_426a_63be_47a7_b6ac_3eaa_c82e_2f6f);
    pub const UNIT_STOPPED: Self = Self(0x9d1a_aa27_d601_40bd_9636_5438_aad2_0286);

    const HEX_DIGITS: usize = 32;

    pub fn add_match(self, journal: &mut Journal) -> io::Result<()> {
        journal.match_add("MESSAGE_ID", self.to_string())?;
        Ok(())
    }

    #[must_use]
    pub fn follow_match(self) -> String {
        format!("MESSAGE_ID={self}")
    }
}

impl Display for MessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{:0width$x}", self.0, width = Self::HEX_DIGITS)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedMessageId(String);

impl Display for MalformedMessageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "message id {:?} is not {} hex digits",
            self.0,
            MessageId::HEX_DIGITS
        )
    }
}

impl FromStr for MessageId {
    type Err = MalformedMessageId;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || MalformedMessageId(text.to_owned());
        if text.len() != Self::HEX_DIGITS || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(malformed());
        }
        u128::from_str_radix(text, 16)
            .map(Self)
            .map_err(|_| malformed())
    }
}

impl TryFrom<String> for MessageId {
    type Error = MalformedMessageId;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalTransport {
    Audit,
    Driver,
    Syslog,
    Journal,
    Stdout,
    Kernel,
}

impl JournalTransport {
    fn field_value(self) -> &'static str {
        match self {
            JournalTransport::Audit => "audit",
            JournalTransport::Driver => "driver",
            JournalTransport::Syslog => "syslog",
            JournalTransport::Journal => "journal",
            JournalTransport::Stdout => "stdout",
            JournalTransport::Kernel => "kernel",
        }
    }

    pub fn add_match(self, journal: &mut Journal) -> io::Result<()> {
        journal.match_add("_TRANSPORT", self.field_value())?;
        Ok(())
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "u8")]
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

    #[must_use]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownSyslogLevel(u8);

impl Display for UnknownSyslogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "syslog level {} is beyond debug", self.0)
    }
}

impl TryFrom<u8> for JournalPriority {
    type Error = UnknownSyslogLevel;

    fn try_from(level: u8) -> Result<Self, UnknownSyslogLevel> {
        Self::MOST_URGENT_FIRST
            .get(usize::from(level))
            .copied()
            .ok_or(UnknownSyslogLevel(level))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwningUnit {
    System(String),
    User(String),
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingUnit {
    #[serde(rename = "_SYSTEMD_UNIT")]
    system: Option<String>,
    #[serde(rename = "_SYSTEMD_USER_UNIT")]
    user: Option<String>,
}

impl LoggingUnit {
    #[must_use]
    pub fn owning_unit(self) -> Option<OwningUnit> {
        OwningUnit::innermost_of(self.system, self.user)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubjectUnit {
    #[serde(rename = "UNIT")]
    system: Option<String>,
    #[serde(rename = "USER_UNIT")]
    user: Option<String>,
}

impl SubjectUnit {
    #[must_use]
    pub fn owning_unit(self) -> Option<OwningUnit> {
        OwningUnit::innermost_of(self.system, self.user)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoredumpUnit {
    #[serde(rename = "COREDUMP_UNIT")]
    system: Option<String>,
    #[serde(rename = "COREDUMP_USER_UNIT")]
    user: Option<String>,
}

impl CoredumpUnit {
    #[must_use]
    pub fn owning_unit(self) -> Option<OwningUnit> {
        OwningUnit::innermost_of(self.system, self.user)
    }
}

impl OwningUnit {
    fn innermost_of(system: Option<String>, user: Option<String>) -> Option<Self> {
        user.map(OwningUnit::User)
            .or_else(|| system.map(OwningUnit::System))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            OwningUnit::System(name) | OwningUnit::User(name) => name,
        }
    }

    #[must_use]
    pub fn logged_by_match(&self) -> String {
        match self {
            OwningUnit::System(name) => format!("_SYSTEMD_UNIT={name}"),
            OwningUnit::User(name) => format!("_SYSTEMD_USER_UNIT={name}"),
        }
    }
}

#[cfg(test)]
pub fn journal_record<'a>(fields: impl IntoIterator<Item = (&'a str, &'a str)>) -> JournalRecord {
    fields
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
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
        assert!(window.drop_expired(now));
        assert_eq!(window.events, [Stamped(minutes(10)), Stamped(minutes(30))]);
        assert_eq!(
            window.time_until_next_expiry(now),
            Some(Duration::from_mins(5))
        );
        assert!(!window.drop_expired(now));
    }

    #[derive(Debug, Deserialize)]
    struct PriorityRecord {
        #[serde(rename = "PRIORITY")]
        priority: JournalPriority,
    }

    #[test]
    fn severity_follows_the_syslog_level_names_and_other_levels_are_malformed() {
        let severity_of = |priority: &str| {
            decode_record::<PriorityRecord>(journal_record([("PRIORITY", priority)]))
                .ok()
                .map(|record| record.priority.severity())
        };
        assert_eq!(
            ["2", "3", "4", "5", "6", "8", "err"].map(severity_of),
            [
                Some(Severity::Critical),
                Some(Severity::Warning),
                Some(Severity::Warning),
                Some(Severity::Normal),
                Some(Severity::Low),
                None,
                None
            ]
        );
    }

    #[derive(Debug, Deserialize)]
    struct CatalogedRecord {
        #[serde(rename = "MESSAGE_ID")]
        message_id: Option<MessageId>,
        #[serde(rename = "_TRANSPORT")]
        transport: JournalTransport,
        #[serde(flatten)]
        unit: LoggingUnit,
    }

    #[test]
    fn optional_and_flattened_fields_decode_from_the_record_text() {
        let record: CatalogedRecord = decode_record(journal_record([
            ("MESSAGE_ID", "fc2e22bc6ee647b6b90729ab34a250b1"),
            ("_TRANSPORT", "journal"),
            ("_SYSTEMD_UNIT", "user@1000.service"),
            ("_SYSTEMD_USER_UNIT", "syncthing.service"),
        ]))
        .expect("a well-formed record");
        assert_eq!(record.message_id, Some(MessageId::COREDUMP));
        assert_eq!(record.transport, JournalTransport::Journal);
        assert_eq!(
            record.unit.owning_unit(),
            Some(OwningUnit::User("syncthing.service".to_owned()))
        );

        let uncataloged: CatalogedRecord =
            decode_record(journal_record([("_TRANSPORT", "kernel")]))
                .expect("a record without catalog id or unit");
        assert_eq!(uncataloged.message_id, None);
        assert_eq!(uncataloged.unit.owning_unit(), None);
    }

    #[test]
    fn message_ids_print_as_journalctl_matches_and_reject_other_lengths() {
        assert_eq!(
            MessageId::KERNEL_OOM_KILL.follow_match(),
            "MESSAGE_ID=fe6faa94e7774663a0da52717891d8ef"
        );
        assert!("fe6faa94".parse::<MessageId>().is_err());
        assert!(
            "+e6faa94e7774663a0da52717891d8ef"
                .parse::<MessageId>()
                .is_err()
        );
    }
}
