use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    mem,
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
use futures::{
    Stream,
    future::{join_all, try_join_all},
};
use icalendar::Calendar;
use tokio::sync::Mutex;

use crate::{
    SnapshotDelivery, SnapshotSource,
    caldav_tasks_events::{
        caldav_calendar_discovery::{
            CaldavLogin, CalendarCollection, CalendarHome, ChangeTracking, connect_calendar_home,
        },
        caldav_event_query::{EventWindow, UpcomingEvent, query_upcoming_events},
        caldav_home_listing::SubscribedCalendar,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, query_open_tasks},
        nextcloud_notify_push::{PushLogin, activity_pings},
        webcal_feed_expansion::download_feed_events,
    },
    delivery_cadence_drivers::{AdaptiveCadence, PeriodicPolling, TriggeredRereadCadence},
    secret_file_reads::read_secret,
    stream_operators::StreamOperators,
};

#[derive(Debug, Clone)]
pub enum Error {
    Secret,
    Rejected,
    NoCurrentUserPrincipal,
    NoCalendarHomeSet,
    WindowBeyondCalendar,
    ListingUnparseable(roxmltree::Error),
    Dav(Arc<fast_dav_rs::Error>),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Secret => write!(f, "password unreadable"),
            Error::Rejected => write!(f, "user or password rejected (HTTP 401)"),
            Error::NoCurrentUserPrincipal => write!(f, "server names no current-user-principal"),
            Error::NoCalendarHomeSet => write!(f, "principal has no calendar-home-set"),
            Error::WindowBeyondCalendar => write!(f, "upcoming_days reaches past the calendar"),
            Error::ListingUnparseable(error) => {
                write!(f, "calendar home listing unparseable: {error}")
            }
            Error::Dav(error) => write!(f, "{error}"),
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
            other => Error::Dav(Arc::new(other)),
        }
    }
}

impl From<roxmltree::Error> for Error {
    fn from(error: roxmltree::Error) -> Self {
        Error::ListingUnparseable(error)
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
    pub change_poll_interval: Duration,
    pub push_safety_recheck_interval: Duration,
    pub subscription_download_interval: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaldavAgenda {
    pub events: Vec<UpcomingEvent>,
    pub tasks: Vec<OpenTask>,
    pub calendar_count: usize,
    pub provider: CaldavProvider,
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

impl SnapshotSource for CaldavAgenda {
    type Error = Error;
    type StreamInputs = CaldavAccount;

    const NAME: &'static str = "CalDAV agenda";

    fn delivery_stream(
        account: CaldavAccount,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let push_live = Arc::new(AtomicBool::new(false));
        let push_login = PushLogin {
            server_url: account.server_url.clone(),
            user: account.user.clone(),
            password: account.password.clone(),
        };
        let change_polling = PeriodicPolling::every(account.change_poll_interval);
        let push_recheck = PeriodicPolling::every(account.push_safety_recheck_interval);
        let cadence_live = push_live.clone();
        let poller = Arc::new(Mutex::new(AgendaPoller::new(account)));
        AdaptiveCadence {
            current_cadence: move || {
                if cadence_live.load(Ordering::Relaxed) {
                    TriggeredRereadCadence::LivePushWithRecheck(push_recheck)
                } else {
                    TriggeredRereadCadence::PeriodicPolling(change_polling)
                }
            },
        }
        .triggered_deliveries(activity_pings(push_login, push_live), move || {
            let poller = poller.clone();
            async move { poller.lock().await.poll().await }
        })
        .repeats_as_unchanged()
    }
}

enum HomeConnection {
    Disconnected,
    Connected {
        home: Box<CalendarHome>,
        password: String,
    },
}

struct FetchedCalendar {
    href: String,
    change_tracking: ChangeTracking,
    window: EventWindow,
    events: Vec<UpcomingEvent>,
    tasks: Vec<OpenTask>,
}

impl FetchedCalendar {
    fn still_current(&self, calendar: &CalendarCollection, window: EventWindow) -> bool {
        self.window == window
            && calendar
                .change_tracking
                .unchanged_since(&self.change_tracking)
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
            href: calendar.href.clone(),
            change_tracking: calendar.change_tracking.clone(),
            window,
            events,
            tasks,
        })
    }
}

struct FetchedSubscription {
    href: String,
    downloaded_at: Instant,
    window: EventWindow,
    events: Vec<UpcomingEvent>,
}

impl FetchedSubscription {
    fn still_fresh(&self, window: EventWindow, refresh_interval: Duration) -> bool {
        self.window == window && self.downloaded_at.elapsed() < refresh_interval
    }

    async fn first_download(
        http_client: &reqwest::Client,
        subscription: &SubscribedCalendar,
        window: EventWindow,
    ) -> Self {
        Self {
            href: subscription.href.clone(),
            downloaded_at: Instant::now(),
            window,
            events: downloaded_or_logged(http_client, subscription, window)
                .await
                .unwrap_or_default(),
        }
    }

    async fn redownloaded(
        self,
        http_client: &reqwest::Client,
        subscription: &SubscribedCalendar,
        window: EventWindow,
    ) -> Self {
        Self {
            downloaded_at: Instant::now(),
            window,
            events: downloaded_or_logged(http_client, subscription, window)
                .await
                .unwrap_or(self.events),
            href: self.href,
        }
    }
}

async fn downloaded_or_logged(
    http_client: &reqwest::Client,
    subscription: &SubscribedCalendar,
    window: EventWindow,
) -> Option<Vec<UpcomingEvent>> {
    match download_feed_events(http_client, subscription, window).await {
        Ok(events) => Some(events),
        Err(error) => {
            tracing::warn!("calendar subscription {}: {error}", subscription.name);
            None
        }
    }
}

struct AgendaPoller {
    account: CaldavAccount,
    http_client: reqwest::Client,
    connection: HomeConnection,
    fetched_calendars: HashMap<String, FetchedCalendar>,
    fetched_subscriptions: HashMap<String, FetchedSubscription>,
}

impl AgendaPoller {
    fn new(account: CaldavAccount) -> Self {
        Self {
            account,
            http_client: reqwest::Client::new(),
            connection: HomeConnection::Disconnected,
            fetched_calendars: HashMap::new(),
            fetched_subscriptions: HashMap::new(),
        }
    }

    async fn poll(&mut self) -> Result<CaldavAgenda, Error> {
        let Some(password) = read_secret(&self.account.password) else {
            return Err(Error::Secret);
        };
        let home = self.connected_home(password).await?;
        let refreshed = self.refresh(&home).await;
        if refreshed.is_err() {
            self.connection = HomeConnection::Disconnected;
        }
        refreshed
    }

    async fn connected_home(&mut self, password: String) -> Result<CalendarHome, Error> {
        if let HomeConnection::Connected {
            home,
            password: known_password,
        } = &self.connection
            && *known_password == password
        {
            return Ok(CalendarHome::clone(home));
        }
        let home = connect_calendar_home(CaldavLogin {
            server_url: &self.account.server_url,
            user: &self.account.user,
            password: &password,
        })
        .await?;
        self.connection = HomeConnection::Connected {
            home: Box::new(home.clone()),
            password,
        };
        Ok(home)
    }

    async fn refresh(&mut self, home: &CalendarHome) -> Result<CaldavAgenda, Error> {
        let listing = home.home_listing().await?;
        let calendars = home.list_calendars().await?;
        let window = EventWindow {
            first_day: Local::now().date_naive(),
            days: self.account.upcoming_days,
        };
        let mut previous_calendars = mem::take(&mut self.fetched_calendars);
        let calendar_fetches = calendars
            .iter()
            .filter(|calendar| listing.shows(&calendar.href))
            .map(|calendar| {
                let cached = previous_calendars.remove(&calendar.href);
                async move {
                    match cached {
                        Some(cached) if cached.still_current(calendar, window) => Ok(cached),
                        _ => FetchedCalendar::fetch(home, calendar, window).await,
                    }
                }
            });
        let fetched_calendars = try_join_all(calendar_fetches).await?;
        let mut previous_subscriptions = mem::take(&mut self.fetched_subscriptions);
        let http_client = &self.http_client;
        let refresh_interval =
            PeriodicPolling::every(self.account.subscription_download_interval).poll_period();
        let subscription_downloads = listing.subscribed_calendars.iter().map(|subscription| {
            let cached = previous_subscriptions.remove(&subscription.href);
            async move {
                match cached {
                    Some(cached) if cached.still_fresh(window, refresh_interval) => cached,
                    Some(cached) => cached.redownloaded(http_client, subscription, window).await,
                    None => {
                        FetchedSubscription::first_download(http_client, subscription, window).await
                    }
                }
            }
        });
        let fetched_subscriptions = join_all(subscription_downloads).await;
        fetched_calendars.into_iter().for_each(|fetched| {
            self.fetched_calendars.insert(fetched.href.clone(), fetched);
        });
        fetched_subscriptions.into_iter().for_each(|fetched| {
            self.fetched_subscriptions
                .insert(fetched.href.clone(), fetched);
        });
        Ok(self.assembled_agenda(Local::now()))
    }

    fn assembled_agenda(&self, now: DateTime<Local>) -> CaldavAgenda {
        let mut events: Vec<UpcomingEvent> = self
            .fetched_calendars
            .values()
            .flat_map(|calendar| calendar.events.iter())
            .chain(
                self.fetched_subscriptions
                    .values()
                    .flat_map(|subscription| subscription.events.iter()),
            )
            .filter(|event| !event.has_ended(now))
            .cloned()
            .collect();
        events.sort_by_key(|event| event.start.starts_at());
        let mut tasks: Vec<OpenTask> = self
            .fetched_calendars
            .values()
            .flat_map(|calendar| calendar.tasks.iter())
            .cloned()
            .collect();
        tasks.sort_by(OpenTask::earliest_due_first);
        CaldavAgenda {
            events,
            tasks,
            calendar_count: self.fetched_calendars.len() + self.fetched_subscriptions.len(),
            provider: CaldavProvider::detected(
                &self.account.server_url,
                self.fetched_calendars.keys().map(String::as_str),
            ),
        }
    }
}
