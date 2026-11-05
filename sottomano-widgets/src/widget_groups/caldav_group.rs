mod open_tasks_reading;
mod upcoming_events_reading;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::{SecretPaths, widget_groups::caldav_account::CaldavAccountConfig};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        caldav_agenda::{self, CaldavAccount},
        caldav_open_tasks::OpenTaskList,
        caldav_server_provider::CaldavProvider,
        caldav_upcoming_events::UpcomingEventList,
    },
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::group_item::GroupItem,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaldavTarget {
    UpcomingEvents,
    OpenTasks,
}

impl From<CaldavTarget> for HoverTarget {
    fn from(target: CaldavTarget) -> Self {
        HoverTarget::Caldav(target)
    }
}

impl From<caldav_agenda::Error> for Section {
    fn from(error: caldav_agenda::Error) -> Self {
        Section::error(error)
    }
}

pub struct CaldavGroup {
    upcoming_events: LatestSnapshot<UpcomingEventList>,
    open_tasks: LatestSnapshot<OpenTaskList>,
    password: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CaldavMessage {
    UpcomingEvents(Box<SnapshotDelivery<UpcomingEventList>>),
    OpenTasks(Box<SnapshotDelivery<OpenTaskList>>),
    Expansion(WidgetGroupExpansionMessage),
}

impl CaldavGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            upcoming_events: LatestSnapshot::default(),
            open_tasks: LatestSnapshot::default(),
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
        match (self.account(config), buses) {
            (Some(account), Some(buses)) => {
                let online_account = OnlineSourceInputs {
                    buses: buses.clone(),
                    source_inputs: account,
                };
                Subscription::batch([
                    snapshot_subscription::<UpcomingEventList>(online_account.clone())
                        .map(|delivery| CaldavMessage::UpcomingEvents(Box::new(delivery))),
                    snapshot_subscription::<OpenTaskList>(online_account)
                        .map(|delivery| CaldavMessage::OpenTasks(Box::new(delivery))),
                ])
            }
            _ => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: CaldavMessage, clock: Clock) -> Task<CaldavMessage> {
        match message {
            CaldavMessage::UpcomingEvents(delivery) => {
                self.upcoming_events.receive(*delivery, clock);
            }
            CaldavMessage::OpenTasks(delivery) => self.open_tasks.receive(*delivery, clock),
            CaldavMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: CaldavTarget,
        config: &CaldavAccountConfig,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            CaldavTarget::UpcomingEvents if self.account(config).is_some() => {
                self.upcoming_events.hover_card_while_pending(clock)
            }
            CaldavTarget::UpcomingEvents => self.upcoming_events.hover_card(clock),
            CaldavTarget::OpenTasks => self.open_tasks.hover_card(clock),
        }
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
    ) -> WidgetGroupContent<'_, CaldavMessage> {
        let upcoming_events_item = if self.account(config).is_some() {
            self.upcoming_events
                .group_item_while_pending(CaldavTarget::UpcomingEvents)
        } else {
            self.upcoming_events
                .group_item(CaldavTarget::UpcomingEvents)
        };
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Caldav,
            &self.expansion,
            CaldavMessage::Expansion,
            [
                upcoming_events_item.map(GroupItem::previewed_on_handle),
                self.open_tasks.group_item(CaldavTarget::OpenTasks),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::Icon {
            glyph: self.account_handle_glyph(),
        })
    }
}
