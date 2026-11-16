use chrono::Locale;

use crate::{
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        offline_internet_indicator::SnapshotPlaceholder,
    },
    widget_groups::caldav_group::{
        open_tasks_indicator::{open_tasks_hover_card, open_tasks_indicator},
        upcoming_events_indicator::{
            UPCOMING_EVENT_LIST_FALLBACK_GLYPH, upcoming_events_hover_card,
            upcoming_events_indicator,
        },
    },
};
mod open_tasks_indicator;
mod upcoming_events_indicator;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        caldav_agenda::CaldavAccount, caldav_open_tasks::OpenTaskList,
        caldav_server_provider::CaldavProvider, caldav_upcoming_events::UpcomingEventList,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{SecretPaths, widget_groups::caldav_account::CaldavAccountConfig};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const OPEN_TASKS_GLYPH: Phosphor = Phosphor::ListChecks(Weight::Regular);

pub struct CaldavGroup {
    upcoming_events: SubscribedSnapshot<UpcomingEventList>,
    open_tasks: SubscribedSnapshot<OpenTaskList>,
    password: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum CaldavMessage {
    UpcomingEvents(Box<SnapshotDelivery<UpcomingEventList>>),
    OpenTasks(Box<SnapshotDelivery<OpenTaskList>>),
}

impl CaldavGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            upcoming_events: SubscribedSnapshot::default(),
            open_tasks: SubscribedSnapshot::default(),
            password: secrets.caldav_password.clone(),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    fn account(&self, config: &CaldavAccountConfig) -> Option<CaldavAccount> {
        Some(CaldavAccount {
            server_url: config.configured_server_url()?.to_owned(),
            user: config.configured_user()?.to_owned(),
            password: self.password.clone()?,
            upcoming_days: config.upcoming_days,
            change_poll_interval: config.change_poll_interval,
            push_safety_recheck_interval: config.push_safety_recheck_interval,
            subscription_download_interval: config.subscription_download_interval,
        })
    }

    pub fn subscription(
        &self,
        config: &CaldavAccountConfig,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<CaldavMessage> {
        let Some(account) = self.account(config) else {
            return Subscription::none();
        };
        let Some(buses) = buses else {
            return Subscription::none();
        };
        let online_account = OnBus {
            buses: buses.clone(),
            inputs: account,
        };
        Subscription::batch([
            snapshot_subscription::<UpcomingEventList>(online_account.clone())
                .map(|delivery| CaldavMessage::UpcomingEvents(Box::new(delivery))),
            snapshot_subscription::<OpenTaskList>(online_account)
                .map(|delivery| CaldavMessage::OpenTasks(Box::new(delivery))),
        ])
    }

    pub fn update(&mut self, message: CaldavMessage, clock: Clock) -> Task<CaldavMessage> {
        match message {
            CaldavMessage::UpcomingEvents(delivery) => {
                self.upcoming_events.receive(*delivery, clock);
            }
            CaldavMessage::OpenTasks(delivery) => self.open_tasks.receive(*delivery, clock),
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        expansion_message: WidgetGroupExpansionMessage,
    ) -> Task<CaldavMessage> {
        self.expansion.update(expansion_message);
        Task::none()
    }

    fn account_handle_glyph(&self) -> IconGlyph {
        let provider = self
            .upcoming_events
            .get()
            .map(|upcoming_events| &upcoming_events.provider)
            .or_else(|| self.open_tasks.get().map(|open_tasks| &open_tasks.provider));
        match provider {
            Some(CaldavProvider::Nextcloud(_)) => SimpleIcon::Nextcloud.into(),
            Some(CaldavProvider::Icloud) => SimpleIcon::iCloud.into(),
            Some(CaldavProvider::GoogleCalendar) => SimpleIcon::GoogleCalendar.into(),
            Some(CaldavProvider::Zoho) => SimpleIcon::Zoho.into(),
            Some(CaldavProvider::MailboxOrg) => SimpleIcon::mailbox.into(),
            Some(CaldavProvider::Unrecognised) | None => Phosphor::Notebook(Weight::Regular).into(),
        }
    }

    pub fn group_content(
        &self,
        config: &CaldavAccountConfig,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_> {
        let mut rows = Vec::new();
        match &self.upcoming_events {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let events_indicator = upcoming_events_indicator(held_snapshot, clock, locale);
                if events_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_upcoming_events_indicator(events_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let events_indicator = troubled_snapshot.shown_over(
                    upcoming_events_indicator(&troubled_snapshot.held_snapshot, clock, locale),
                    clock,
                );
                if events_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_upcoming_events_indicator(events_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_upcoming_events_indicator(
                    placeholder.indicator(UPCOMING_EVENT_LIST_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                if self.account(config).is_some() {
                    rows.push(self.wired_upcoming_events_indicator(SnapshotPlaceholder::<
                        UpcomingEventList,
                    >::awaiting_indicator(
                        UPCOMING_EVENT_LIST_FALLBACK_GLYPH,
                    )));
                }
            }
            SubscribedSnapshot::SourceUnavailable => {}
        }
        match &self.open_tasks {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let tasks_indicator = open_tasks_indicator(held_snapshot, clock);
                if tasks_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_open_tasks_indicator(tasks_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let tasks_indicator = troubled_snapshot.shown_over(
                    open_tasks_indicator(&troubled_snapshot.held_snapshot, clock),
                    clock,
                );
                if tasks_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_open_tasks_indicator(tasks_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.wired_open_tasks_indicator(placeholder.indicator(OPEN_TASKS_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Caldav,
            GroupSummary::glyph(self.account_handle_glyph(), IndicatorState::Idle),
            &self.expansion,
            WidgetGroupInteraction::Caldav,
            rows,
        )
    }

    fn wired_upcoming_events_indicator<'a>(
        &'a self,
        events_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        events_indicator.with_hover_card(Box::new(move |card_clock| {
            self.upcoming_events
                .sections_described_by(|snapshot| upcoming_events_hover_card(snapshot, card_clock))
        }))
    }

    fn wired_open_tasks_indicator<'a>(
        &'a self,
        tasks_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        tasks_indicator.with_hover_card(Box::new(move |card_clock| {
            self.open_tasks
                .sections_described_by(|snapshot| open_tasks_hover_card(snapshot, card_clock))
        }))
    }
}
