use std::{
    collections::HashMap,
    mem,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use futures::{Stream, future::join_all};

use crate::{
    SnapshotDelivery, SnapshotSource,
    caldav_tasks_events::{
        caldav_agenda::{
            AgendaSnapshotPolling, CaldavAccount, CalendarEntryQuery, CalendarHomePoller, Error,
            push_or_poll_deliveries,
        },
        caldav_calendar_discovery::{CalendarCollection, CalendarHome},
        caldav_event_query::{EventPhase, EventWindow, UpcomingEvent, query_upcoming_events},
        caldav_home_listing::SubscribedCalendar,
        caldav_server_provider::CaldavProvider,
        webcal_feed_expansion::download_feed_events,
    },
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpcomingEventList {
    pub events: Vec<UpcomingEvent>,
    pub calendar_count: usize,
    pub failed_subscription_names: Vec<String>,
    pub provider: CaldavProvider,
}

impl UpcomingEventList {
    #[must_use]
    pub fn next_event(&self, now: DateTime<Local>) -> Option<&UpcomingEvent> {
        self.events
            .iter()
            .find(|event| event.phase(now) != EventPhase::Ended)
    }
}

impl CalendarEntryQuery for UpcomingEvent {
    fn held_by(calendar: &CalendarCollection) -> bool {
        calendar.holds_events
    }

    async fn queried(
        home: &CalendarHome,
        calendar: &CalendarCollection,
        window: EventWindow,
    ) -> Result<Vec<Self>, Error> {
        query_upcoming_events(home, calendar, window).await
    }
}

struct FetchedSubscription {
    href: String,
    calendar_name: String,
    downloaded_at: Instant,
    window: EventWindow,
    download_failed: bool,
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
        let downloaded_events = downloaded_or_logged(http_client, subscription, window).await;
        Self {
            href: subscription.href.clone(),
            calendar_name: subscription.name.clone(),
            downloaded_at: Instant::now(),
            window,
            download_failed: downloaded_events.is_none(),
            events: downloaded_events.unwrap_or_default(),
        }
    }

    async fn redownloaded(
        self,
        http_client: &reqwest::Client,
        subscription: &SubscribedCalendar,
        window: EventWindow,
    ) -> Self {
        let downloaded_events = downloaded_or_logged(http_client, subscription, window).await;
        Self {
            calendar_name: subscription.name.clone(),
            downloaded_at: Instant::now(),
            window,
            download_failed: downloaded_events.is_none(),
            events: downloaded_events.unwrap_or(self.events),
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

struct UpcomingEventPoller {
    calendar_home: CalendarHomePoller<UpcomingEvent>,
    http_client: reqwest::Client,
    fetched_subscriptions: HashMap<String, FetchedSubscription>,
}

impl UpcomingEventPoller {
    async fn refresh_subscriptions(
        &mut self,
        subscriptions: &[SubscribedCalendar],
        window: EventWindow,
    ) {
        let mut previous_subscriptions = mem::take(&mut self.fetched_subscriptions);
        let http_client = &self.http_client;
        let refresh_interval =
            PeriodicPolling::every(self.calendar_home.account().subscription_download_interval)
                .poll_period();
        let subscription_downloads = subscriptions.iter().map(|subscription| {
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
        join_all(subscription_downloads)
            .await
            .into_iter()
            .for_each(|fetched| {
                self.fetched_subscriptions
                    .insert(fetched.href.clone(), fetched);
            });
    }
}

impl AgendaSnapshotPolling for UpcomingEventPoller {
    type Snapshot = UpcomingEventList;

    async fn polled_snapshot(&mut self) -> Result<UpcomingEventList, Error> {
        let polled_home = self.calendar_home.poll().await?;
        self.refresh_subscriptions(
            &polled_home.listing.subscribed_calendars,
            polled_home.window,
        )
        .await;
        let now = Local::now();
        let mut events: Vec<UpcomingEvent> = self
            .calendar_home
            .fetched_entries()
            .chain(
                self.fetched_subscriptions
                    .values()
                    .flat_map(|subscription| subscription.events.iter()),
            )
            .filter(|event| event.phase(now) != EventPhase::Ended)
            .cloned()
            .collect();
        events.sort_by_key(|event| event.start.starts_at());
        let mut failed_subscription_names: Vec<String> = self
            .fetched_subscriptions
            .values()
            .filter(|subscription| subscription.download_failed)
            .map(|subscription| subscription.calendar_name.clone())
            .collect();
        failed_subscription_names.sort();
        tracing::debug!(
            "upcoming events: {} kept from {} calendars",
            events.len(),
            self.calendar_home.fetched_calendar_count(),
        );
        Ok(UpcomingEventList {
            events,
            failed_subscription_names,
            calendar_count: self.calendar_home.fetched_calendar_count()
                + self.fetched_subscriptions.len(),
            provider: polled_home.provider,
        })
    }
}

impl SnapshotSource for UpcomingEventList {
    type Error = Error;
    type StreamInputs = OnlineSourceInputs<CaldavAccount>;

    const NAME: &'static str = "CalDAV events";

    fn delivery_stream(
        account: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        push_or_poll_deliveries(account, |account| UpcomingEventPoller {
            calendar_home: CalendarHomePoller::new(account),
            http_client: reqwest::Client::new(),
            fetched_subscriptions: HashMap::new(),
        })
    }
}
