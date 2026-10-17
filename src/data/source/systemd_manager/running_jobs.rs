use std::{
    collections::HashMap,
    pin::Pin,
    time::{Duration, SystemTime},
};

use iced::futures::{Stream, StreamExt, future::join_all, stream};
use zbus::{proxy::CacheProperties, zvariant::OwnedObjectPath};

use super::{
    JobRow, ManagerProxy, SystemdOnBus, SystemdUnitScope, manager_signal, signals_matching,
    sleep_until, state_changed_at,
};
use crate::{
    data::source::{DeliveryCadence, ReadingSource, SourceDelivery, drop_unchanged_deliveries},
    message_subscriptions::connected_until_failure,
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
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

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

impl ReadingSource for SlowJobs {
    type Error = zbus::Error;
    type StreamInputs = SlowJobsWatch;

    const NAME: &'static str = "slow systemd jobs";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(watch: SlowJobsWatch) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let scope = watch.systemd.scope;
        drop_unchanged_deliveries(
            connected_until_failure(watch_running_jobs(
                watch.systemd.connection(),
                watch.slow_after,
            ))
            .map(move |listed| listed.map(|jobs| SlowJobs { jobs, scope }))
            .map(SourceDelivery::Reading),
        )
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

enum JobSignal {
    Queued(u32),
    Removed(u32),
}

impl JobSignal {
    fn of_message(message: &zbus::Message) -> Option<Self> {
        let header = message.header();
        let body = message.body();
        match header.member()?.as_str() {
            "JobNew" => body
                .deserialize::<(u32, OwnedObjectPath, String)>()
                .ok()
                .map(|(id, _, _)| JobSignal::Queued(id)),
            "JobRemoved" => body
                .deserialize::<(u32, OwnedObjectPath, String, String)>()
                .ok()
                .map(|(id, _, _, _)| JobSignal::Removed(id)),
            _ => None,
        }
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
    manager.subscribe().await?;
    let job_signals = signals_matching(
        &connection,
        [manager_signal("JobNew")?, manager_signal("JobRemoved")?],
    )
    .await?
    .ready_chunks(64)
    .map(|messages| {
        messages
            .iter()
            .filter_map(|message| message.as_ref().ok().and_then(JobSignal::of_message))
            .collect()
    });
    Ok(stream::unfold(
        RunningJobWatch {
            manager,
            job_signals: Box::pin(job_signals),
            queued_at: HashMap::new(),
            running: None,
            slow_after,
        },
        RunningJobWatch::next_report,
    ))
}

struct RunningJobWatch {
    manager: ManagerProxy<'static>,
    job_signals: Pin<Box<dyn Stream<Item = Vec<JobSignal>> + Send>>,
    queued_at: HashMap<u32, SystemTime>,
    running: Option<Vec<RunningJob>>,
    slow_after: Duration,
}

impl RunningJobWatch {
    async fn next_report(mut self) -> Option<(zbus::Result<Vec<SlowJob>>, Self)> {
        let listed = match self.running.take() {
            None => running_jobs(&self.manager, &self.queued_at, &[]).await,
            Some(running) => {
                let next_slow = next_slow_deadline(&running, self.slow_after, SystemTime::now());
                tokio::select! {
                    signals = self.job_signals.next() => {
                        self.remember(signals?);
                        running_jobs(&self.manager, &self.queued_at, &running).await
                    }
                    () = sleep_until(next_slow) => Ok(running),
                }
            }
        };
        let report = listed.map(|running| {
            let slow = slow_jobs(&running, self.slow_after, SystemTime::now());
            self.running = Some(running);
            slow
        });
        Some((report, self))
    }

    fn remember(&mut self, signals: Vec<JobSignal>) {
        let now = SystemTime::now();
        for signal in signals {
            match signal {
                JobSignal::Queued(id) => {
                    self.queued_at.insert(id, now);
                }
                JobSignal::Removed(id) => {
                    self.queued_at.remove(&id);
                }
            }
        }
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
            let started_at = job.started_at?;
            (started_at + slow_after <= now).then(|| SlowJob {
                unit: job.unit.clone(),
                job_type: job.job_type.clone(),
                started_at,
            })
        })
        .collect()
}

async fn running_jobs(
    manager: &ManagerProxy<'static>,
    queued_at: &HashMap<u32, SystemTime>,
    already_running: &[RunningJob],
) -> zbus::Result<Vec<RunningJob>> {
    let rows = manager.list_jobs().await?;
    let connection = manager.inner().connection();
    Ok(join_all(
        rows.into_iter()
            .filter(|row| row.state == RUNNING_JOB_STATE)
            .map(|row| {
                let known = already_running.iter().find(|job| job.id == row.id).cloned();
                let queued_at = queued_at.get(&row.id).copied();
                async move {
                    match known {
                        Some(known) => known,
                        None => running_job(connection, row, queued_at).await,
                    }
                }
            }),
    )
    .await)
}

async fn running_job(
    connection: &zbus::Connection,
    row: JobRow,
    queued_at: Option<SystemTime>,
) -> RunningJob {
    let unit_state_changed_at = state_changed_at(connection, row.unit_path).await;
    RunningJob {
        id: row.id,
        unit: row.unit,
        job_type: row.job_type,
        started_at: queued_at.max(unit_state_changed_at),
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
