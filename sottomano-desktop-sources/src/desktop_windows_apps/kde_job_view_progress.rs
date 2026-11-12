use std::{
    collections::HashMap,
    future::ready,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::{Stream, StreamExt};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use zbus::{
    Connection, connection,
    fdo::{self, RequestNameFlags, RequestNameReply},
    object_server::ObjectServer,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::system_protocol_clients::{
    dbus_desktop_buses::ServingConnectionStreams, protocol_peer_reconnects::resubscribed,
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

#[derive(Default)]
struct Job {
    app: String,
    percent: Option<u8>,
    total: u64,
    processed: u64,
}

impl Job {
    fn apply(&mut self, properties: &HashMap<String, OwnedValue>) {
        if let Some(percent) = number(properties, "percent") {
            self.percent = u8::try_from(percent.min(100)).ok();
        }
        if let Some(total) = number(properties, "totalBytes") {
            self.total = total;
        }
        if let Some(processed) = number(properties, "processedBytes") {
            self.processed = processed;
        }
    }

    fn progress(&self) -> u8 {
        if let Some(percent) = self.percent {
            return percent.min(100);
        }
        if self.total == 0 {
            return 0;
        }
        let ratio = self.processed.min(self.total) * 100 / self.total;
        u8::try_from(ratio).unwrap_or(100)
    }
}

fn number(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let value = properties.get(key)?;
    let read = |convert: fn(OwnedValue) -> Option<u64>| value.try_clone().ok().and_then(convert);
    read(|v| u64::try_from(v).ok())
        .or_else(|| read(|v| u32::try_from(v).ok().map(u64::from)))
        .or_else(|| read(|v| i64::try_from(v).ok().and_then(|n| u64::try_from(n).ok())))
        .or_else(|| read(|v| i32::try_from(v).ok().and_then(|n| u64::try_from(n).ok())))
}

fn mean(percents: &[u8]) -> u8 {
    let sum: u32 = percents.iter().copied().map(u32::from).sum();
    let count = u32::try_from(percents.len()).unwrap_or(1).max(1);
    u8::try_from(sum / count).unwrap_or(100)
}

struct Tracker {
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

async fn open_view(
    tracker: &Arc<Tracker>,
    object_server: &ObjectServer,
    app: String,
) -> fdo::Result<OwnedObjectPath> {
    let id = tracker.open(app);
    let path = OwnedObjectPath::try_from(format!("{PATH}/JobView_{id}"))
        .map_err(|e| fdo::Error::Failed(e.to_string()))?;
    let view = JobView {
        id,
        path: path.clone(),
        tracker: tracker.clone(),
    };
    let modern = JobViewV3 {
        id,
        path: path.clone(),
        tracker: tracker.clone(),
    };
    object_server.at(&path, view).await?;
    object_server.at(&path, modern).await?;
    Ok(path)
}

struct JobViewV3 {
    id: u64,
    path: OwnedObjectPath,
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewV3")]
#[allow(clippy::needless_pass_by_value)]
impl JobViewV3 {
    #[zbus(name = "update")]
    fn update(&self, properties: HashMap<String, OwnedValue>) {
        self.tracker.amend(self.id, |job| job.apply(&properties));
    }

    #[zbus(name = "terminate")]
    async fn terminate(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        error_code: u32,
        error_message: String,
        hints: HashMap<String, OwnedValue>,
    ) {
        let _ = (error_code, error_message, hints);
        self.tracker.close(self.id);
        let _ = object_server.remove::<Self, _>(&self.path).await;
        let _ = object_server.remove::<JobView, _>(&self.path).await;
    }
}

struct JobView {
    id: u64,
    path: OwnedObjectPath,
    tracker: Arc<Tracker>,
}

#[zbus::interface(name = "org.kde.JobViewV2")]
#[allow(clippy::unused_self, clippy::needless_pass_by_value)]
impl JobView {
    #[zbus(name = "terminate")]
    async fn terminate(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        error_message: String,
    ) {
        let _ = error_message;
        self.tracker.close(self.id);
        let _ = object_server.remove::<Self, _>(&self.path).await;
    }

    #[zbus(name = "setPercent")]
    fn set_percent(&self, percent: u32) {
        let percent = u8::try_from(percent.min(100)).unwrap_or(100);
        self.tracker
            .amend(self.id, |job| job.percent = Some(percent));
    }

    #[zbus(name = "setTotalAmount")]
    fn set_total_amount(&self, amount: u64, unit: String) {
        if unit != "bytes" && unit != "files" && unit != "dirs" && unit != "items" {
            return;
        }
        self.tracker.amend(self.id, |job| job.total = amount);
    }

    #[zbus(name = "setProcessedAmount")]
    fn set_processed_amount(&self, amount: u64, unit: String) {
        if unit != "bytes" && unit != "files" && unit != "dirs" && unit != "items" {
            return;
        }
        self.tracker.amend(self.id, |job| job.processed = amount);
    }

    #[zbus(name = "setInfoMessage")]
    fn set_info_message(&self, message: String) {
        let _ = message;
    }

    #[zbus(name = "setDescriptionField")]
    fn set_description_field(&self, number: u32, name: String, value: String) -> bool {
        let _ = (number, name, value);
        false
    }

    #[zbus(name = "clearDescriptionField")]
    fn clear_description_field(&self, number: u32) {
        let _ = number;
    }

    #[zbus(name = "setSuspended")]
    fn set_suspended(&self, suspended: bool) {
        let _ = suspended;
    }

    #[zbus(name = "setSpeed")]
    fn set_speed(&self, bytes_per_second: u64) {
        let _ = bytes_per_second;
    }

    #[zbus(name = "setDestUrl")]
    fn set_dest_url(&self, dest_url: OwnedValue) {
        let _ = dest_url;
    }

    #[zbus(name = "setError")]
    fn set_error(&self, error_code: u32) {
        let _ = error_code;
    }
}

pub fn job_progress() -> impl Stream<Item = JobProgress> + Send {
    resubscribed(Duration::from_secs(1), (), |()| async {
        Ok::<_, zbus::Error>(serve().await?.map(Ok::<_, zbus::Error>))
    })
    .filter_map(|delivery| ready(delivery.ok()))
}

async fn serve() -> zbus::Result<impl Stream<Item = JobProgress>> {
    let (states, updates) = watch::channel(JobProgress::default());
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

    Ok(WatchStream::from_changes(updates).while_serving(connection))
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
