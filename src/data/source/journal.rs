use std::{
    fs, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iced::futures::{
    Stream, StreamExt,
    future::join_all,
    stream::{self, BoxStream},
};
use serde::Deserialize;
use systemd::journal::{Journal, JournalRecord, OpenOptions};
use tokio::sync::mpsc;
use zbus::{
    message, proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

const HORIZON: Duration = Duration::from_secs(3600);
const AGE_OUT_RECHECK: Duration = Duration::from_secs(60);
const WARNING_PRIORITY: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KernelSeverity {
    Critical,
    Error,
    Warning,
}

impl KernelSeverity {
    fn from_priority(priority: u8) -> Option<Self> {
        match priority {
            0..=2 => Some(KernelSeverity::Critical),
            3 => Some(KernelSeverity::Error),
            WARNING_PRIORITY => Some(KernelSeverity::Warning),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KernelEntry {
    pub at: SystemTime,
    pub message: String,
    pub severity: KernelSeverity,
}

pub fn kernel_errors() -> mpsc::Receiver<Vec<KernelEntry>> {
    let (tx, rx) = mpsc::channel(16);
    thread::spawn(move || {
        let Some(mut journal) = open_kernel_journal() else {
            tracing::warn!("journal: kernel journal open failed");
            return;
        };
        if tx.blocking_send(scan_kernel_errors(&mut journal)).is_err() {
            return;
        }
        loop {
            match journal.wait(Some(AGE_OUT_RECHECK)) {
                Ok(_) => {
                    if tx.blocking_send(scan_kernel_errors(&mut journal)).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    tracing::warn!("journal: kernel wait ended: {e}");
                    return;
                }
            }
        }
    });
    rx
}

fn open_kernel_journal() -> Option<Journal> {
    let mut journal = OpenOptions::default().system(true).open().ok()?;
    journal.match_add("_TRANSPORT", "kernel").ok()?;
    for priority in 0..=WARNING_PRIORITY {
        journal.match_add("PRIORITY", priority.to_string()).ok()?;
    }
    Some(journal)
}

fn scan_kernel_errors(journal: &mut Journal) -> Vec<KernelEntry> {
    let Some(cutoff_usec) = horizon_cutoff_usec() else {
        return Vec::new();
    };
    if journal.seek_realtime_usec(cutoff_usec).is_err() {
        return Vec::new();
    }
    let mut entries = Vec::new();
    while let Ok(Some(record)) = journal.next_entry() {
        let at = journal.timestamp().unwrap_or_else(|_| SystemTime::now());
        entries.extend(kernel_entry(&record, at));
    }
    entries
}

fn kernel_entry(record: &JournalRecord, at: SystemTime) -> Option<KernelEntry> {
    let severity = record
        .get("PRIORITY")
        .and_then(|priority| priority.parse().ok())
        .and_then(KernelSeverity::from_priority)?;
    Some(KernelEntry {
        at,
        message: record.get("MESSAGE")?.clone(),
        severity,
    })
}

fn horizon_cutoff_usec() -> Option<u64> {
    let cutoff = SystemTime::now().checked_sub(HORIZON)?;
    u64::try_from(cutoff.duration_since(UNIX_EPOCH).ok()?.as_micros()).ok()
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1",
    gen_blocking = false
)]
trait Manager {
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
    fn subscribe(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Unit {
    #[zbus(property)]
    fn state_change_timestamp_monotonic(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1",
    gen_blocking = false
)]
trait Service {
    #[zbus(property)]
    fn result(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn exec_main_status(&self) -> zbus::Result<i32>;
}

#[derive(Debug, Deserialize, Type)]
#[allow(dead_code)]
struct UnitRow {
    name: String,
    description: String,
    load_state: String,
    active_state: String,
    sub_state: String,
    following: String,
    unit_path: OwnedObjectPath,
    job_id: u32,
    job_type: String,
    job_path: OwnedObjectPath,
}

#[derive(Debug, Clone)]
pub struct FailedUnitInfo {
    pub name: String,
    pub result: String,
    pub status: i32,
    pub at: Option<SystemTime>,
}

pub fn failed_stream(per_user: bool) -> impl Stream<Item = Vec<FailedUnitInfo>> {
    stream::once(watch_failed(per_user)).flatten()
}

async fn watch_failed(user: bool) -> BoxStream<'static, Vec<FailedUnitInfo>> {
    let connection = match connect(user).await {
        Ok(connection) => connection,
        Err(e) => {
            tracing::warn!("journal: systemd connect failed: {e}");
            return stream::empty().boxed();
        }
    };
    let manager = match ManagerProxy::new(&connection).await {
        Ok(manager) => manager,
        Err(e) => {
            tracing::warn!("journal: systemd manager proxy failed: {e}");
            return stream::empty().boxed();
        }
    };
    if let Err(e) = manager.subscribe().await {
        tracing::warn!("journal: systemd subscribe failed: {e}");
    }
    let signals = match systemd_signals(&connection).await {
        Ok(signals) => signals,
        Err(e) => {
            tracing::warn!("journal: systemd signal match failed: {e}");
            return stream::empty().boxed();
        }
    };
    let initial = stream::once(failed_units(manager.clone()));
    let updates = signals.then(move |()| failed_units(manager.clone()));
    initial.chain(updates).boxed()
}

async fn connect(user: bool) -> zbus::Result<zbus::Connection> {
    if user {
        zbus::Connection::session().await
    } else {
        zbus::Connection::system().await
    }
}

async fn systemd_signals(
    connection: &zbus::Connection,
) -> zbus::Result<impl Stream<Item = ()> + use<>> {
    let rule = zbus::MatchRule::builder()
        .msg_type(message::Type::Signal)
        .sender("org.freedesktop.systemd1")?
        .build();
    let stream = zbus::MessageStream::for_match_rule(rule, connection, Some(64)).await?;
    Ok(stream.ready_chunks(64).map(|_| ()))
}

async fn failed_units(manager: ManagerProxy<'static>) -> Vec<FailedUnitInfo> {
    let Ok(rows) = manager.list_units_filtered(&["failed"]).await else {
        return Vec::new();
    };
    let uptime = uptime();
    let now = SystemTime::now();
    let connection = manager.inner().connection();
    join_all(
        rows.into_iter()
            .map(|row| failed_unit(connection, row, uptime, now)),
    )
    .await
}

async fn failed_unit(
    connection: &zbus::Connection,
    row: UnitRow,
    uptime: Option<Duration>,
    now: SystemTime,
) -> FailedUnitInfo {
    let UnitDetails {
        result,
        status,
        changed_us,
    } = details(connection, row.unit_path)
        .await
        .unwrap_or_else(|_| UnitDetails::unknown());
    FailedUnitInfo {
        name: row.name,
        result,
        status,
        at: uptime.and_then(|uptime| failed_at(changed_us, uptime, now)),
    }
}

fn failed_at(changed_us: u64, uptime: Duration, now: SystemTime) -> Option<SystemTime> {
    if changed_us == 0 {
        return None;
    }
    let age = uptime.checked_sub(Duration::from_micros(changed_us))?;
    now.checked_sub(age)
}

struct UnitDetails {
    result: String,
    status: i32,
    changed_us: u64,
}

impl UnitDetails {
    fn unknown() -> Self {
        Self {
            result: "failed".to_string(),
            status: 0,
            changed_us: 0,
        }
    }
}

async fn details(
    connection: &zbus::Connection,
    path: OwnedObjectPath,
) -> zbus::Result<UnitDetails> {
    let unit = UnitProxy::builder(connection)
        .path(path.clone())?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let service = ServiceProxy::builder(connection)
        .path(path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let unknown = UnitDetails::unknown();
    Ok(UnitDetails {
        result: service.result().await.unwrap_or(unknown.result),
        status: service.exec_main_status().await.unwrap_or(unknown.status),
        changed_us: unit
            .state_change_timestamp_monotonic()
            .await
            .unwrap_or(unknown.changed_us),
    })
}

fn uptime() -> Option<Duration> {
    let content = fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}
