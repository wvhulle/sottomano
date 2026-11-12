use std::time::{Duration, Instant, SystemTime};

use futures::{Stream, StreamExt, future::join_all};
use futures_rx::RxExt;
use zbus::{proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{
    JobRow, ManagerProxy, ServiceProxy, SystemdOnBus, SystemdUnitScope, manager_signal,
    state_changed_at,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, RereadSchedule},
    system_protocol_clients::{
        dbus_change_signals::change_triggers, protocol_peer_reconnects::subscribed,
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
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    #[must_use]
    pub fn follow_log_args(&self) -> Vec<String> {
        self.scope
            .follow_units_log_args(self.jobs.iter().map(|job| job.unit.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SlowJobsWatch {
    pub systemd: SystemdOnBus,
    pub slow_after: Duration,
}

impl SnapshotSource for SlowJobs {
    type Error = zbus::Error;
    type StreamInputs = SlowJobsWatch;

    const NAME: &'static str = "Slow systemd jobs";

    fn delivery_stream(watch: SlowJobsWatch) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let scope = watch.systemd.scope;
        let slow_after = watch.slow_after;
        subscribed(watch.systemd.connection(), move |connection| {
            watch_running_jobs(connection, slow_after)
        })
        .map(move |listed| listed.map(|jobs| SlowJobs { jobs, scope }))
        .map(SnapshotDelivery::live_push)
        .distinct_until_changed()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunningJob {
    id: u32,
    unit: String,
    job_type: String,
    started_at: Option<SystemTime>,
    runs_oneshot_service: bool,
}

impl RunningJob {
    fn runs_until_work_finishes(&self) -> bool {
        self.runs_oneshot_service && self.job_type == START_JOB_TYPE
    }

    fn slow_from(&self, slow_after: Duration) -> Option<SystemTime> {
        if self.runs_until_work_finishes() {
            return None;
        }
        Some(self.started_at? + slow_after)
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
    .await?
    .map(drop);
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
        + time
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
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
            let started_at = job.started_at?;
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
    Ok(join_all(
        rows.into_iter()
            .filter(|row| row.state == RUNNING_JOB_STATE)
            .map(|row| running_job(connection, row)),
    )
    .await)
}

async fn running_job(connection: &zbus::Connection, row: JobRow) -> RunningJob {
    RunningJob {
        runs_oneshot_service: is_oneshot_service(connection, row.unit_path.clone()).await,
        started_at: state_changed_at(connection, row.unit_path).await,
        id: row.id,
        unit: row.unit,
        job_type: row.job_type,
    }
}

async fn is_oneshot_service(connection: &zbus::Connection, unit_path: OwnedObjectPath) -> bool {
    let Ok(builder) = ServiceProxy::builder(connection).path(unit_path) else {
        return false;
    };
    let Ok(service) = builder.cache_properties(CacheProperties::No).build().await else {
        return false;
    };
    service
        .service_startup_kind()
        .await
        .is_ok_and(|startup_kind| startup_kind == ONESHOT_SERVICE_KIND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_earliest_job_not_yet_slow_decides_the_next_wake_up() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let job = |id: u32, started_secs_ago: Option<u64>| RunningJob {
            id,
            unit: format!("unit{id}.service"),
            job_type: "stop".to_owned(),
            started_at: started_secs_ago.map(|ago| start - Duration::from_secs(ago)),
            runs_oneshot_service: false,
        };
        let running = [
            job(1, Some(30)),
            job(2, Some(4)),
            job(3, Some(7)),
            job(4, None),
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
            started_at: Some(start - Duration::from_secs(60)),
            runs_oneshot_service: true,
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
