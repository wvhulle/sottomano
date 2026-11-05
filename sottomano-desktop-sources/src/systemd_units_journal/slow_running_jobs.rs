use std::time::{Duration, Instant, SystemTime};

use futures::{Stream, StreamExt, future::join_all};
use zbus::proxy::CacheProperties;

use super::{
    JobRow, ManagerProxy, SystemdOnBus, SystemdUnitScope, manager_signal, signals_matching,
    state_changed_at,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{StreamOperators, connected_until_failure, reread_on_trigger_or_deadline},
};

const RUNNING_JOB_STATE: &str = "running";

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
        connected_until_failure(watch_running_jobs(
            watch.systemd.connection(),
            watch.slow_after,
        ))
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
}

impl RunningJob {
    fn slow_from(&self, slow_after: Duration) -> Option<SystemTime> {
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
    let job_signals = signals_matching(
        &connection,
        [manager_signal("JobNew")?, manager_signal("JobRemoved")?],
    )
    .await?
    .map(drop);
    let listings = reread_on_trigger_or_deadline(
        job_signals,
        move || {
            let manager = manager.clone();
            async move { running_jobs(&manager).await }
        },
        move |listed| {
            let running = listed.as_ref().ok()?;
            next_slow_deadline(running, slow_after, SystemTime::now()).map(instant_at)
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
            (started_at + slow_after <= now).then(|| SlowJob {
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
        started_at: state_changed_at(connection, row.unit_path).await,
        id: row.id,
        unit: row.unit,
        job_type: row.job_type,
    }
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
}
