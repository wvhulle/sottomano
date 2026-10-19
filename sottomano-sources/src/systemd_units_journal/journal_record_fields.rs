use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    str::FromStr,
};

use serde::Deserialize;
use sottomano_config::severity::Severity;
use systemd::journal::Journal;

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
mod tests {
    use super::*;
    use crate::system_protocol_clients::journal_follow::{decode_record, journal_record};

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
