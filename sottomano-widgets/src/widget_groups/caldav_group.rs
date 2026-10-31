mod open_tasks_reading;
mod upcoming_events_reading;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::{SecretPaths, widget_groups::caldav_account::CaldavAccountConfig};
use sottomano_sources::{
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
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::Glyph,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace, WidgetGroup},
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
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CaldavMessage {
    UpcomingEvents(Box<SnapshotDelivery<UpcomingEventList>>),
    OpenTasks(Box<SnapshotDelivery<OpenTaskList>>),
    Expansion(ExpansionMessage),
}

impl WidgetGroup for CaldavGroup {
    const TITLE: &'static str = "Calendar";
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
            expansion: GroupExpansion::default(),
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
    pub fn tooltip_for(&self, target: CaldavTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            CaldavTarget::UpcomingEvents => self.upcoming_events.tooltip(clock),
            CaldavTarget::OpenTasks => self.open_tasks.tooltip(clock),
        }
    }

    fn account_handle_glyph(&self) -> Glyph {
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

    pub fn group_content(&self) -> GroupContent<'_, CaldavMessage> {
        Self::expandable_group(
            &self.expansion,
            CaldavMessage::Expansion,
            [
                self.upcoming_events
                    .group_item(CaldavTarget::UpcomingEvents)
                    .map(GroupItem::previewed_on_handle),
                self.open_tasks.group_item(CaldavTarget::OpenTasks),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: self.account_handle_glyph(),
            handle: GroupHandle::Caldav,
        })
    }
}
