use std::time::{Duration, Instant, SystemTime};

use futures::{Stream, StreamExt, stream};
use zbus::{proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{
    JobRow, ManagerProxy, ServiceProxy, SystemdUnitScope, UNIT_PROPERTY_READS_IN_FLIGHT,
    UnitStateChange, manager_signal, state_changed_at,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, RereadSchedule, SnapshotDeliveryStreams},
    system_protocol_clients::{
        dbus_change_signals::change_triggers, dbus_desktop_buses::OnBus,
        protocol_peer_reconnects::subscribed,
    },
};

const RUNNING_JOB_STATE: &str = "running";
const START_JOB_TYPE: &str = "start";
const ONESHOT_SERVICE_KIND: &str = "oneshot";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlowJob {
    pub unit: String,
    pub job_type: String,
    pub started_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlowJobs {
    pub jobs: Vec<SlowJob>,
    pub scope: SystemdUnitScope,
}

impl SlowJobs {
    #[must_use]
    pub fn follow_log_args(&self) -> Vec<String> {
        self.scope
            .follow_units_log_args(self.jobs.iter().map(|job| job.unit.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlowJobsWatch {
    pub systemd: OnBus<SystemdUnitScope>,
    pub slow_after: Duration,
}

impl SnapshotSource for SlowJobs {
    type Error = zbus::Error;
    type StreamInputs = SlowJobsWatch;

    const NAME: &'static str = "Slow systemd jobs";

    fn delivery_stream(watch: SlowJobsWatch) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let scope = watch.systemd.inputs;
        let slow_after = watch.slow_after;
        subscribed(watch.systemd.connection(), move |connection| {
            watch_running_jobs(connection, slow_after)
        })
        .map(move |listed| listed.map(|jobs| SlowJobs { jobs, scope }))
        .map(SnapshotDelivery::live_push)
        .repeats_as_unchanged()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunningJob {
    id: u32,
    unit: String,
    job_type: String,
    started_at: UnitStateChange,
    startup_kind: UnitStartupKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitStartupKind {
    OneshotService,
    NotOneshotService,
}

impl RunningJob {
    fn slow_from(&self, slow_after: Duration) -> Option<SystemTime> {
        if self.startup_kind == UnitStartupKind::OneshotService && self.job_type == START_JOB_TYPE {
            return None;
        }
        let UnitStateChange::At(started_at) = self.started_at else {
            return None;
        };
        Some(started_at + slow_after)
    }
}

async fn watch_running_jobs(
    connection: zbus::Connection,
    slow_after: Duration,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<SlowJob>>>> {
    let manager = ManagerProxy::builder(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    manager.ensure_subscribed().await?;
    let job_signals = change_triggers(
        &connection,
        [manager_signal("JobNew")?, manager_signal("JobRemoved")?],
    )
    .await?;
    let listings = job_signals.paced_rereads(
        Duration::ZERO,
        move || {
            let manager = manager.clone();
            async move { running_jobs(&manager).await }
        },
        move |listed| {
            let Ok(running) = listed.as_ref() else {
                return RereadSchedule::WaitForTrigger;
            };
            next_slow_deadline(running, slow_after, SystemTime::now())
                .map_or(RereadSchedule::WaitForTrigger, |deadline| {
                    RereadSchedule::RecheckAt(instant_at(deadline))
                })
        },
    );
    Ok(listings.map(move |listed| {
        listed.map(|running| slow_jobs(&running, slow_after, SystemTime::now()))
    }))
}

fn instant_at(time: SystemTime) -> Instant {
    Instant::now()
        + match time.duration_since(SystemTime::now()) {
            Ok(ahead) => ahead,
            Err(_already_passed) => Duration::ZERO,
        }
}

fn next_slow_deadline(
    running: &[RunningJob],
    slow_after: Duration,
    now: SystemTime,
) -> Option<SystemTime> {
    running
        .iter()
        .filter_map(|job| job.slow_from(slow_after))
        .filter(|slow_from| *slow_from > now)
        .min()
}

fn slow_jobs(running: &[RunningJob], slow_after: Duration, now: SystemTime) -> Vec<SlowJob> {
    running
        .iter()
        .filter_map(|job| {
            let UnitStateChange::At(started_at) = job.started_at else {
                return None;
            };
            (job.slow_from(slow_after)? <= now).then(|| SlowJob {
                unit: job.unit.clone(),
                job_type: job.job_type.clone(),
                started_at,
            })
        })
        .collect()
}

async fn running_jobs(manager: &ManagerProxy<'static>) -> zbus::Result<Vec<RunningJob>> {
    let rows = manager.list_jobs().await?;
    let connection = manager.inner().connection();
    Ok(stream::iter(
        rows.into_iter()
            .filter(|row| row.state == RUNNING_JOB_STATE),
    )
    .map(|row| running_job(connection, row))
    .buffered(UNIT_PROPERTY_READS_IN_FLIGHT)
    .collect()
    .await)
}

async fn running_job(connection: &zbus::Connection, row: JobRow) -> RunningJob {
    RunningJob {
        startup_kind: unit_startup_kind(connection, row.unit_path.clone()).await,
        started_at: state_changed_at(connection, row.unit_path).await,
        id: row.id,
        unit: row.unit,
        job_type: row.job_type,
    }
}

async fn unit_startup_kind(
    connection: &zbus::Connection,
    unit_path: OwnedObjectPath,
) -> UnitStartupKind {
    let Ok(builder) = ServiceProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .path(unit_path)
    else {
        return UnitStartupKind::NotOneshotService;
    };
    let Ok(service) = builder.build().await else {
        return UnitStartupKind::NotOneshotService;
    };
    match service.service_startup_kind().await {
        Ok(startup_kind) if startup_kind == ONESHOT_SERVICE_KIND => UnitStartupKind::OneshotService,
        Ok(_) | Err(_) => UnitStartupKind::NotOneshotService,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_earliest_job_not_yet_slow_decides_the_next_wake_up() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let job = |id: u32, started_at: UnitStateChange| RunningJob {
            id,
            unit: format!("unit{id}.service"),
            job_type: "stop".to_owned(),
            started_at,
            startup_kind: UnitStartupKind::NotOneshotService,
        };
        let started_secs_ago = |ago: u64| UnitStateChange::At(start - Duration::from_secs(ago));
        let running = [
            job(1, started_secs_ago(30)),
            job(2, started_secs_ago(4)),
            job(3, started_secs_ago(7)),
            job(4, UnitStateChange::Unrecorded),
        ];
        let slow_after = Duration::from_secs(10);
        assert_eq!(
            next_slow_deadline(&running, slow_after, start),
            Some(start + Duration::from_secs(3))
        );
        let slow: Vec<String> = slow_jobs(&running, slow_after, start)
            .into_iter()
            .map(|job| job.unit)
            .collect();
        assert_eq!(slow, ["unit1.service"]);
    }

    #[test]
    fn starting_a_oneshot_service_is_never_slow_but_stopping_one_is() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let oneshot_job = |id: u32, job_type: &str| RunningJob {
            id,
            unit: format!("oneshot{id}.service"),
            job_type: job_type.to_owned(),
            started_at: UnitStateChange::At(start - Duration::from_secs(60)),
            startup_kind: UnitStartupKind::OneshotService,
        };
        let running = [oneshot_job(1, "start"), oneshot_job(2, "stop")];
        let slow_after = Duration::from_secs(10);
        let slow: Vec<String> = slow_jobs(&running, slow_after, start)
            .into_iter()
            .map(|job| job.unit)
            .collect();
        assert_eq!(slow, ["oneshot2.service"]);
        assert_eq!(
            next_slow_deadline(&running[..1], slow_after, start - Duration::from_secs(55)),
            None
        );
    }
}
