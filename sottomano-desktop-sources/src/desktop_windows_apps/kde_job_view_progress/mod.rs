mod kde_job_view_objects;

#[expect(
    clippy::disallowed_types,
    reason = "imported for the job table the zbus interface mutates through &self"
)]
use std::{
    collections::HashMap,
    future::ready,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::{Stream, StreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use zbus::{
    Connection, connection,
    fdo::{self, RequestNameFlags, RequestNameReply},
    object_server::ObjectServer,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

use self::kde_job_view_objects::open_view;
use crate::system_protocol_clients::{
    dbus_desktop_buses::served_while_connected, protocol_peer_reconnects::subscribed,
};

const PATH: &str = "/JobViewServer";
const SERVER_NAME: &str = "org.kde.JobViewServer";
const KUISERVER_NAME: &str = "org.kde.kuiserver";
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobProgress {
    by_app: HashMap<String, u8>,
}

impl JobProgress {
    #[must_use]
    pub fn get(&self, app_id: &str) -> Option<u8> {
        self.by_app.get(app_id).copied()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum JobPercent {
    #[default]
    Unreported,
    Reported(u8),
}

#[derive(Default)]
struct Job {
    app: String,
    percent: JobPercent,
    total: u64,
    processed: u64,
}

impl Job {
    fn apply(&mut self, properties: &HashMap<String, OwnedValue>) {
        if let Some(percent) = number(properties, "percent") {
            self.percent = JobPercent::Reported(
                u8::try_from(percent.min(100)).expect("percent is capped at one hundred"),
            );
        }
        if let Some(total) = number(properties, "totalBytes") {
            self.total = total;
        }
        if let Some(processed) = number(properties, "processedBytes") {
            self.processed = processed;
        }
    }

    fn progress(&self) -> u8 {
        if let JobPercent::Reported(percent) = self.percent {
            return percent.min(100);
        }
        if self.total == 0 {
            return 0;
        }
        let ratio = self.processed.min(self.total) * 100 / self.total;
        u8::try_from(ratio).expect("a ratio of processed to total capped at 100 fits a byte")
    }
}

fn number(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let value = properties.get(key)?;
    let converted_number = match &**value {
        Value::U64(unsigned_number) => Some(*unsigned_number),
        Value::U32(unsigned_number) => Some(u64::from(*unsigned_number)),
        Value::I64(signed_number) if *signed_number >= 0 => Some(signed_number.unsigned_abs()),
        Value::I32(signed_number) if *signed_number >= 0 => {
            Some(u64::from(signed_number.unsigned_abs()))
        }
        _ => None,
    };
    if converted_number.is_none() {
        tracing::warn!("job view: {key} is not a non-negative number: {value:?}");
    }
    converted_number
}

fn mean(percents: &[u8]) -> u8 {
    let sum: usize = percents.iter().copied().map(usize::from).sum();
    u8::try_from(sum / percents.len().max(1)).expect("a mean of percentages fits a byte")
}

struct Tracker {
    #[expect(
        clippy::disallowed_types,
        reason = "zbus serves the interface through &self, so the job table it updates needs a \
                  lock"
    )]
    jobs: Mutex<HashMap<u64, Job>>,
    states: watch::Sender<JobProgress>,
    next_id: AtomicU64,
}

impl Tracker {
    fn open(&self, app: String) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.jobs.lock().expect("job registry").insert(
            id,
            Job {
                app,
                ..Job::default()
            },
        );
        self.publish();
        id
    }

    fn amend(&self, id: u64, change: impl FnOnce(&mut Job)) {
        if let Some(job) = self.jobs.lock().expect("job registry").get_mut(&id) {
            change(job);
        }
        self.publish();
    }

    fn close(&self, id: u64) {
        self.jobs.lock().expect("job registry").remove(&id);
        self.publish();
    }

    fn publish(&self) {
        let next = self.snapshot();
        self.states.send_if_modified(|current| {
            if *current == next {
                return false;
            }
            *current = next;
            true
        });
    }

    fn snapshot(&self) -> JobProgress {
        let jobs = self.jobs.lock().expect("job registry");
        JobProgress {
            by_app: jobs
                .values()
                .filter(|job| !job.app.is_empty())
                .fold(HashMap::<String, Vec<u8>>::new(), |mut grouped, job| {
                    grouped
                        .entry(job.app.clone())
                        .or_default()
                        .push(job.progress());
                    grouped
                })
                .into_iter()
                .map(|(app, percents)| (app, mean(&percents)))
                .collect(),
        }
    }
}

struct JobViewServerV2 {
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewServerV2")]
#[allow(clippy::needless_pass_by_value)]
impl JobViewServerV2 {
    #[zbus(name = "requestView")]
    async fn request_view(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        desktop_entry: String,
        capabilities: i32,
        hints: HashMap<String, OwnedValue>,
    ) -> fdo::Result<OwnedObjectPath> {
        let _ = (capabilities, hints);
        open_view(&self.tracker, object_server, desktop_entry).await
    }
}

struct JobViewServerV1 {
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewServer")]
#[allow(clippy::needless_pass_by_value)]
impl JobViewServerV1 {
    #[zbus(name = "requestView")]
    async fn request_view(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        app_name: String,
        app_icon_name: String,
        capabilities: i32,
    ) -> fdo::Result<OwnedObjectPath> {
        let _ = (app_icon_name, capabilities);
        open_view(&self.tracker, object_server, app_name).await
    }
}

pub fn job_progress() -> impl Stream<Item = JobProgress> + Send {
    subscribed((), |()| async {
        Ok::<_, zbus::Error>(serve().await?.map(Ok::<_, zbus::Error>))
    })
    .filter_map(|delivery| {
        ready(
            delivery
                .inspect_err(|serve_failure| tracing::warn!("job view: {serve_failure}"))
                .ok(),
        )
    })
}

async fn serve() -> zbus::Result<impl Stream<Item = JobProgress>> {
    let (states, updates) = watch::channel(JobProgress::default());
    #[expect(
        clippy::disallowed_types,
        reason = "zbus serves the interface through &self, so the job table it updates needs a \
                  lock"
    )]
    let tracker = Arc::new(Tracker {
        jobs: Mutex::default(),
        states,
        next_id: AtomicU64::new(1),
    });
    let connection = connection::Builder::session()?
        .serve_at(
            PATH,
            JobViewServerV2 {
                tracker: tracker.clone(),
            },
        )?
        .serve_at(
            PATH,
            JobViewServerV1 {
                tracker: tracker.clone(),
            },
        )?
        .build()
        .await?;

    claim(&connection, SERVER_NAME).await?;
    claim(&connection, KUISERVER_NAME).await?;

    Ok(served_while_connected(
        WatchStream::from_changes(updates),
        connection,
    ))
}

async fn claim(connection: &Connection, name: &str) -> zbus::Result<()> {
    let reply = connection
        .request_name_with_flags(
            name,
            RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement,
        )
        .await?;
    match reply {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => Ok(()),
        RequestNameReply::InQueue | RequestNameReply::Exists => Err(zbus::Error::Failure(format!(
            "{name} is owned by another service that disallowed replacement"
        ))),
    }
}
