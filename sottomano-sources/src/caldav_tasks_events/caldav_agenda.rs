use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use fast_dav_rs::CalendarObject;
use futures::{Stream, StreamExt};
use icalendar::Calendar;
use tokio::sync::Mutex;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    caldav_tasks_events::{
        caldav_calendar_discovery::{
            CaldavLogin, CalendarCollection, CalendarHome, connect_calendar_home,
        },
        caldav_event_query::{EventWindow, UpcomingEvent, query_upcoming_events},
        caldav_task_query::{OpenTask, query_open_tasks},
        nextcloud_notify_push::{PushLogin, activity_pings},
        nextcloud_web_links::NextcloudWebRoot,
    },
    secret_file_reads::read_secret,
    stream_operators::{StreamOperators, reread_on_trigger_or_deadline},
};

const RECHECK_WITHOUT_PUSH: Duration = Duration::from_mins(1);
const RECHECK_WHILE_PUSHED: Duration = Duration::from_mins(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Secret,
    Rejected,
    Discovery(&'static str),
    Dav(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "password unreadable"),
            Error::Rejected => write!(f, "user or password rejected (HTTP 401)"),
            Error::Discovery(reason) => write!(f, "{reason}"),
            Error::Dav(detail) => write!(f, "{detail}"),
        }
    }
}

impl From<fast_dav_rs::Error> for Error {
    fn from(error: fast_dav_rs::Error) -> Self {
        match error {
            fast_dav_rs::Error::UnexpectedStatus { status, .. }
            | fast_dav_rs::Error::UnexpectedStatusWithDav { status, .. }
                if status.as_u16() == 401 =>
            {
                Error::Rejected
            }
            other => Error::Dav(other.to_string()),
        }
    }
}

pub(super) struct CalendarResource {
    pub href: String,
    pub calendar: Calendar,
}

pub(super) fn parsed_calendar_data(objects: Vec<CalendarObject>) -> Vec<CalendarResource> {
    objects
        .into_iter()
        .filter_map(|object| {
            Some(CalendarResource {
                calendar: Calendar::from_str(object.calendar_data.as_deref()?).ok()?,
                href: object.href,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaldavAccount {
    pub server_url: String,
    pub user: String,
    pub password: PathBuf,
    pub upcoming_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDelivery {
    NextcloudNotifyPush,
    ChangeTokenPolling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaldavAgenda {
    pub events: Vec<UpcomingEvent>,
    pub tasks: Vec<OpenTask>,
    pub calendar_count: usize,
    pub change_delivery: ChangeDelivery,
    pub nextcloud: Option<NextcloudWebRoot>,
}

impl CaldavAgenda {
    #[must_use]
    pub fn next_event(&self, now: DateTime<Local>) -> Option<&UpcomingEvent> {
        self.events.iter().find(|event| !event.has_ended(now))
    }

    #[must_use]
    pub fn overdue_task_count(&self, now: DateTime<Local>) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.is_overdue(now))
            .count()
    }
}

impl ReadingSource for CaldavAgenda {
    type Error = Error;
    type StreamInputs = CaldavAccount;

    const NAME: &'static str = "caldav-agenda";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(RECHECK_WITHOUT_PUSH);

    fn delivery_stream(account: CaldavAccount) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let push_live = Arc::new(AtomicBool::new(false));
        let push_login = PushLogin {
            server_url: account.server_url.clone(),
            user: account.user.clone(),
            password: account.password.clone(),
        };
        let poller = Arc::new(Mutex::new(AgendaPoller::new(account, push_live.clone())));
        let recheck_live = push_live.clone();
        reread_on_trigger_or_deadline(
            activity_pings(push_login, push_live),
            move || {
                let poller = poller.clone();
                async move { poller.lock().await.poll().await }
            },
            move |_| {
                let recheck = if recheck_live.load(Ordering::Relaxed) {
                    RECHECK_WHILE_PUSHED
                } else {
                    RECHECK_WITHOUT_PUSH
                };
                Some(Instant::now() + recheck)
            },
        )
        .map(SourceDelivery::Reading)
        .repeats_as_unchanged()
    }
}

struct ConnectedHome {
    home: CalendarHome,
    password: String,
}

struct FetchedCalendar {
    change_token: Option<String>,
    window: EventWindow,
    events: Vec<UpcomingEvent>,
    tasks: Vec<OpenTask>,
}

impl FetchedCalendar {
    fn still_current(&self, calendar: &CalendarCollection, window: EventWindow) -> bool {
        self.window == window
            && calendar.change_token.is_some()
            && self.change_token == calendar.change_token
    }

    async fn fetch(
        home: &CalendarHome,
        calendar: &CalendarCollection,
        window: EventWindow,
    ) -> Result<Self, Error> {
        let events = if calendar.holds_events {
            query_upcoming_events(home, calendar, window).await?
        } else {
            Vec::new()
        };
        let tasks = if calendar.holds_tasks {
            query_open_tasks(home, calendar).await?
        } else {
            Vec::new()
        };
        Ok(Self {
            change_token: calendar.change_token.clone(),
            window,
            events,
            tasks,
        })
    }
}

struct AgendaPoller {
    account: CaldavAccount,
    push_live: Arc<AtomicBool>,
    connection: Option<ConnectedHome>,
    fetched: HashMap<String, FetchedCalendar>,
}

impl AgendaPoller {
    fn new(account: CaldavAccount, push_live: Arc<AtomicBool>) -> Self {
        Self {
            account,
            push_live,
            connection: None,
            fetched: HashMap::new(),
        }
    }

    async fn poll(&mut self) -> Result<CaldavAgenda, Error> {
        let password = read_secret(&self.account.password).ok_or(Error::Secret)?;
        let home = self.connected_home(password).await?;
        let refreshed = self.refresh(&home).await;
        if refreshed.is_err() {
            self.connection = None;
        }
        refreshed
    }

    async fn connected_home(&mut self, password: String) -> Result<CalendarHome, Error> {
        if let Some(connected) = &self.connection
            && connected.password == password
        {
            return Ok(connected.home.clone());
        }
        let home = connect_calendar_home(CaldavLogin {
            server_url: &self.account.server_url,
            user: &self.account.user,
            password: &password,
        })
        .await?;
        self.connection = Some(ConnectedHome {
            home: home.clone(),
            password,
        });
        Ok(home)
    }

    async fn refresh(&mut self, home: &CalendarHome) -> Result<CaldavAgenda, Error> {
        let calendars = home.list_calendars().await?;
        let window = EventWindow {
            first_day: Local::now().date_naive(),
            days: self.account.upcoming_days,
        };
        let mut current = HashMap::with_capacity(calendars.len());
        for calendar in &calendars {
            let fetched = match self.fetched.remove(&calendar.href) {
                Some(cached) if cached.still_current(calendar, window) => cached,
                _ => FetchedCalendar::fetch(home, calendar, window).await?,
            };
            current.insert(calendar.href.clone(), fetched);
        }
        self.fetched = current;
        Ok(self.assembled_agenda(Local::now()))
    }

    fn assembled_agenda(&self, now: DateTime<Local>) -> CaldavAgenda {
        let mut events: Vec<UpcomingEvent> = self
            .fetched
            .values()
            .flat_map(|calendar| calendar.events.iter())
            .filter(|event| !event.has_ended(now))
            .cloned()
            .collect();
        events.sort_by_key(|event| event.start.starts_at());
        let mut tasks: Vec<OpenTask> = self
            .fetched
            .values()
            .flat_map(|calendar| calendar.tasks.iter())
            .cloned()
            .collect();
        tasks.sort_by(OpenTask::earliest_due_first);
        CaldavAgenda {
            events,
            tasks,
            calendar_count: self.fetched.len(),
            change_delivery: if self.push_live.load(Ordering::Relaxed) {
                ChangeDelivery::NextcloudNotifyPush
            } else {
                ChangeDelivery::ChangeTokenPolling
            },
            nextcloud: self.fetched.keys().find_map(|calendar_href| {
                NextcloudWebRoot::detected(&self.account.server_url, calendar_href)
            }),
        }
    }
}
