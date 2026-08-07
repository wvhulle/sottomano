use std::{
    fs, thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iced::futures::{
    Stream, StreamExt,
    stream::{self, BoxStream},
};
use serde::Deserialize;
use systemd::journal::{Journal, OpenOptions};
use tokio::{sync::mpsc, task::spawn_blocking};
use zbus::{
    blocking, message, proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

const HORIZON: Duration = Duration::from_secs(3600);
const AGE_OUT_RECHECK: Duration = Duration::from_secs(60);
const ERR_PRIORITY: u8 = 3;

#[derive(Debug, Clone)]
pub struct KernelEntry {
    pub at: SystemTime,
    pub message: String,
    pub priority: u8,
}

pub fn kernel_errors() -> mpsc::Receiver<Vec<KernelEntry>> {
    let (tx, rx) = mpsc::channel(16);
    thread::spawn(move || {
        let Some(mut journal) = open_kernel_journal() else {
            eprintln!("journal: kernel journal open failed");
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
                    eprintln!("journal: kernel wait ended: {e}");
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
    for priority in 0..=ERR_PRIORITY {
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
        let (Some(priority), Some(message)) = (
            record
                .get("PRIORITY")
                .and_then(|priority| priority.parse().ok()),
            record.get("MESSAGE"),
        ) else {
            continue;
        };
        entries.push(KernelEntry {
            at: journal.timestamp().unwrap_or_else(|_| SystemTime::now()),
            message: message.clone(),
            priority,
        });
    }
    entries
}

fn horizon_cutoff_usec() -> Option<u64> {
    let cutoff = SystemTime::now().checked_sub(HORIZON)?;
    u64::try_from(cutoff.duration_since(UNIX_EPOCH).ok()?.as_micros()).ok()
}

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Manager {
    fn list_units_filtered(&self, states: &[&str]) -> zbus::Result<Vec<UnitRow>>;
    fn subscribe(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait Unit {
    #[zbus(property)]
    fn state_change_timestamp_monotonic(&self) -> zbus::Result<u64>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Service",
    default_service = "org.freedesktop.systemd1"
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
            eprintln!("journal: systemd connect failed: {e}");
            return stream::empty().boxed();
        }
    };
    if let Ok(manager) = ManagerProxy::new(&connection).await
        && let Err(e) = manager.subscribe().await
    {
        eprintln!("journal: systemd subscribe failed: {e}");
    }
    let signals = match systemd_signals(&connection).await {
        Ok(signals) => signals,
        Err(e) => {
            eprintln!("journal: systemd signal match failed: {e}");
            return stream::empty().boxed();
        }
    };
    let initial = stream::once(failed_units_off_thread(user));
    let updates = signals.then(move |()| failed_units_off_thread(user));
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

async fn failed_units_off_thread(user: bool) -> Vec<FailedUnitInfo> {
    spawn_blocking(move || failed_units(user))
        .await
        .unwrap_or_default()
}

fn failed_units(user: bool) -> Vec<FailedUnitInfo> {
    let connection = if user {
        blocking::Connection::session()
    } else {
        blocking::Connection::system()
    };
    let Ok(connection) = connection else {
        return Vec::new();
    };
    let Ok(manager) = ManagerProxyBlocking::new(&connection) else {
        return Vec::new();
    };
    let Ok(rows) = manager.list_units_filtered(&["failed"]) else {
        return Vec::new();
    };
    let uptime = uptime();
    let now = SystemTime::now();
    rows.into_iter()
        .map(|row| failed_unit(&connection, row, uptime, now))
        .collect()
}

fn failed_unit(
    connection: &blocking::Connection,
    row: UnitRow,
    uptime: Option<Duration>,
    now: SystemTime,
) -> FailedUnitInfo {
    let UnitDetails {
        result,
        status,
        changed_us,
    } = details(connection, &row.unit_path);
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

fn details(connection: &blocking::Connection, path: &OwnedObjectPath) -> UnitDetails {
    let unit = UnitProxyBlocking::builder(connection)
        .path(path.clone())
        .ok()
        .and_then(|builder| builder.cache_properties(CacheProperties::No).build().ok());
    let service = ServiceProxyBlocking::builder(connection)
        .path(path.clone())
        .ok()
        .and_then(|builder| builder.cache_properties(CacheProperties::No).build().ok());
    let changed_us = unit
        .and_then(|unit| unit.state_change_timestamp_monotonic().ok())
        .unwrap_or(0);
    let result = service
        .as_ref()
        .and_then(|service| service.result().ok())
        .unwrap_or_else(|| "failed".to_string());
    let status = service
        .and_then(|service| service.exec_main_status().ok())
        .unwrap_or(0);
    UnitDetails {
        result,
        status,
        changed_us,
    }
}

fn uptime() -> Option<Duration> {
    let content = fs::read_to_string("/proc/uptime").ok()?;
    let secs: f64 = content.split_whitespace().next()?.parse().ok()?;
    Some(Duration::from_secs_f64(secs))
}
