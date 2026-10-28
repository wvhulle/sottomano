mod open_tasks_reading;
mod upcoming_events_reading;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::{SecretPaths, widget_groups::caldav_account::CaldavAccountConfig};
use sottomano_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        caldav_agenda::{self, CaldavAccount, CaldavAgenda},
        caldav_server_provider::CaldavProvider,
    },
};

use self::open_tasks_reading::OpenTasks;
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
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
    agenda: LatestSnapshot<CaldavAgenda>,
    password: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CaldavMessage {
    Agenda(Box<SnapshotDelivery<CaldavAgenda>>),
    Expansion(ExpansionMessage),
}

impl CaldavGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            agenda: LatestSnapshot::default(),
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

    pub fn subscription(&self, config: &CaldavAccountConfig) -> Subscription<CaldavMessage> {
        match self.account(config) {
            Some(account) => snapshot_subscription::<CaldavAgenda>(account)
                .map(|delivery| CaldavMessage::Agenda(Box::new(delivery))),
            None => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: CaldavMessage, clock: Clock) -> Task<CaldavMessage> {
        match message {
            CaldavMessage::Agenda(delivery) => self.agenda.receive(*delivery, clock),
            CaldavMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: CaldavTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            CaldavTarget::UpcomingEvents => self.agenda.tooltip(clock),
            CaldavTarget::OpenTasks => self
                .agenda
                .tooltip_with(OpenTasks(self.agenda.visible_reading()?).tooltip(clock)),
        }
    }

    fn account_handle_glyph(&self) -> Glyph {
        match self.agenda.get().map(|agenda| &agenda.provider) {
            Some(CaldavProvider::Nextcloud(_)) => SimpleIcon::Nextcloud.into(),
            Some(CaldavProvider::Icloud) => SimpleIcon::iCloud.into(),
            Some(CaldavProvider::GoogleCalendar) => SimpleIcon::GoogleCalendar.into(),
            Some(CaldavProvider::Zoho) => SimpleIcon::Zoho.into(),
            Some(CaldavProvider::MailboxOrg) => SimpleIcon::mailbox.into(),
            Some(CaldavProvider::Unrecognised) | None => Phosphor::Notebook(Weight::Regular).into(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, CaldavMessage> {
        let open_tasks = self
            .agenda
            .visible_reading()
            .map(OpenTasks)
            .filter(BarWidget::visible_in_bar)
            .map(|tasks| GroupItem::reading(tasks, CaldavTarget::OpenTasks));
        GroupContent::new(
            &self.expansion,
            CaldavMessage::Expansion,
            [
                self.agenda
                    .group_item(CaldavTarget::UpcomingEvents)
                    .map(GroupItem::previewed_on_handle),
                open_tasks,
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
