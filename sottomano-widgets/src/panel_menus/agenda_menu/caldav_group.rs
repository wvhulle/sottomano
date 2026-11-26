use chrono::Locale;

use crate::group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState};
mod open_tasks_indicator;
mod upcoming_events_indicator;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    caldav_tasks_events::{
        caldav_agenda::CaldavAccount, caldav_open_tasks::OpenTaskList,
        caldav_server_provider::CaldavProvider, caldav_upcoming_events::UpcomingEventList,
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::{
    SecretPaths, panel_menus::agenda_menu::caldav_account::CaldavAccountConfig,
    secrets::StoredSecret,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub struct CaldavGroup {
    upcoming_events: SubscribedSnapshot<UpcomingEventList>,
    open_tasks: SubscribedSnapshot<OpenTaskList>,
    password: StoredSecret,
    expansion: IndicatorGroupExpansion,
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
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    fn account(&self, config: &CaldavAccountConfig) -> Option<CaldavAccount> {
        let StoredSecret::Present(password) = &self.password else {
            return None;
        };
        Some(CaldavAccount {
            server_url: config.configured_server_url()?.to_owned(),
            user: config.configured_user()?.to_owned(),
            password: password.clone(),
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
        expansion_message: IndicatorGroupExpansionMessage,
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
    ) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Caldav,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Caldav.title(),
                self.account_handle_glyph(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            IndicatorGroupInteraction::Caldav,
            [
                self.upcoming_events_listing(config, clock, locale),
                self.open_tasks_listing(config, clock),
            ]
            .into_iter()
            .collect(),
        )
    }
}
