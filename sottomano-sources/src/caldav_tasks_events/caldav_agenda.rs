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
    time::Duration,
};

use chrono::Local;
use fast_dav_rs::CalendarObject;
use futures::{Stream, future::try_join_all};
use icalendar::Calendar;
use tokio::sync::Mutex;

use crate::{
    SnapshotDelivery, SnapshotSource,
    caldav_tasks_events::{
        caldav_calendar_discovery::{
            CaldavLogin, CalendarCollection, CalendarHome, ChangeTracking, connect_calendar_home,
        },
        caldav_event_query::EventWindow,
        caldav_home_listing::CalendarHomeListing,
        caldav_server_provider::CaldavProvider,
        nextcloud_notify_push::{PushLogin, activity_pings},
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

pub(super) trait CalendarEntryQuery: Clone + Send + Sync + 'static {
    fn held_by(calendar: &CalendarCollection) -> bool;

    fn queried(
        home: &CalendarHome,
        calendar: &CalendarCollection,
        window: EventWindow,
    ) -> impl Future<Output = Result<Vec<Self>, Error>> + Send;
}

pub(super) trait AgendaSnapshotPolling: Send + 'static {
    type Snapshot: SnapshotSource<Error = Error> + PartialEq;

    fn polled_snapshot(&mut self) -> impl Future<Output = Result<Self::Snapshot, Error>> + Send;
}

pub(super) fn push_or_poll_deliveries<Poller: AgendaSnapshotPolling>(
    account: &CaldavAccount,
    poller: Poller,
) -> impl Stream<Item = SnapshotDelivery<Poller::Snapshot>> + Send + use<Poller> {
    let push_live = Arc::new(AtomicBool::new(false));
    let push_login = PushLogin {
        server_url: account.server_url.clone(),
        user: account.user.clone(),
        password: account.password.clone(),
    };
    let change_polling = PeriodicPolling::every(account.change_poll_interval);
    let push_recheck = PeriodicPolling::every(account.push_safety_recheck_interval);
    let cadence_live = push_live.clone();
    let poller = Arc::new(Mutex::new(poller));
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
        async move { poller.lock().await.polled_snapshot().await }
    })
    .repeats_as_unchanged()
}

enum HomeConnection {
    Disconnected,
    Connected {
        home: Box<CalendarHome>,
        password: String,
    },
}

struct FetchedCalendar<Entry> {
    href: String,
    change_tracking: ChangeTracking,
    window: EventWindow,
    entries: Vec<Entry>,
}

impl<Entry: CalendarEntryQuery> FetchedCalendar<Entry> {
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
        Ok(Self {
            href: calendar.href.clone(),
            change_tracking: calendar.change_tracking.clone(),
            window,
            entries: Entry::queried(home, calendar, window).await?,
        })
    }
}

pub(super) struct PolledCalendarHome {
    pub listing: CalendarHomeListing,
    pub provider: CaldavProvider,
    pub window: EventWindow,
}

pub(super) struct CalendarHomePoller<Entry> {
    account: CaldavAccount,
    connection: HomeConnection,
    fetched_calendars: HashMap<String, FetchedCalendar<Entry>>,
}

impl<Entry: CalendarEntryQuery> CalendarHomePoller<Entry> {
    pub(super) fn new(account: CaldavAccount) -> Self {
        Self {
            account,
            connection: HomeConnection::Disconnected,
            fetched_calendars: HashMap::new(),
        }
    }

    pub(super) fn account(&self) -> &CaldavAccount {
        &self.account
    }

    pub(super) fn fetched_entries(&self) -> impl Iterator<Item = &Entry> {
        self.fetched_calendars
            .values()
            .flat_map(|calendar| calendar.entries.iter())
    }

    pub(super) fn fetched_calendar_count(&self) -> usize {
        self.fetched_calendars.len()
    }

    pub(super) async fn poll(&mut self) -> Result<PolledCalendarHome, Error> {
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

    async fn refresh(&mut self, home: &CalendarHome) -> Result<PolledCalendarHome, Error> {
        let listing = home.home_listing().await?;
        let calendars = home.list_calendars().await?;
        let window = EventWindow {
            first_day: Local::now().date_naive(),
            days: self.account.upcoming_days,
        };
        let shown_calendars: Vec<&CalendarCollection> = calendars
            .iter()
            .filter(|calendar| listing.shows(&calendar.href))
            .collect();
        let provider = CaldavProvider::detected(
            &self.account.server_url,
            shown_calendars
                .iter()
                .map(|calendar| calendar.href.as_str()),
        );
        let mut previous_calendars = mem::take(&mut self.fetched_calendars);
        let calendar_fetches = shown_calendars
            .into_iter()
            .filter(|calendar| Entry::held_by(calendar))
            .map(|calendar| {
                let cached = previous_calendars.remove(&calendar.href);
                async move {
                    match cached {
                        Some(cached) if cached.still_current(calendar, window) => Ok(cached),
                        _ => FetchedCalendar::fetch(home, calendar, window).await,
                    }
                }
            });
        try_join_all(calendar_fetches)
            .await?
            .into_iter()
            .for_each(|fetched| {
                self.fetched_calendars.insert(fetched.href.clone(), fetched);
            });
        Ok(PolledCalendarHome {
            listing,
            provider,
            window,
        })
    }
}
