mod open_tasks_reading;
mod upcoming_events_reading;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::{SecretPaths, widget_groups::caldav_account::CaldavAccountConfig};
use sottomano_sources::{
    SourceDelivery,
    blocking_background_work::blocking,
    caldav_tasks_events::caldav_agenda::{self, CaldavAccount, CaldavAgenda},
    system_protocol_clients::shell_command_output::blocking::run,
};

use self::open_tasks_reading::OpenTasks;
use crate::{
    background_work::source_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, Tooltip},
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
    agenda: ContinuousReading<CaldavAgenda>,
    password: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CaldavMessage {
    Agenda(Box<SourceDelivery<CaldavAgenda>>),
    Expansion(ExpansionMessage),
    OpenWeb(String),
}

impl CaldavGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(secrets: &SecretPaths) -> Self {
        Self {
            agenda: ContinuousReading::default(),
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
        })
    }

    pub fn subscription(&self, config: &CaldavAccountConfig) -> Subscription<CaldavMessage> {
        match self.account(config) {
            Some(account) => source_subscription::<CaldavAgenda>(account)
                .map(|delivery| CaldavMessage::Agenda(Box::new(delivery))),
            None => Subscription::none(),
        }
    }

    pub fn update(&mut self, message: CaldavMessage, clock: Clock) -> Task<CaldavMessage> {
        match message {
            CaldavMessage::Agenda(delivery) => self.agenda.receive_delivery(*delivery, clock),
            CaldavMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
            CaldavMessage::OpenWeb(url) => {
                return Task::future(blocking("open caldav web", move || {
                    run("xdg-open", &[&url])
                }))
                .discard();
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

    pub fn group_content(&self, config: &CaldavAccountConfig) -> GroupContent<'_, CaldavMessage> {
        let web_url = config.configured_web_url();
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
            .flatten()
            .map(move |item| opened_in_browser(item, web_url)),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::CalendarDots(Weight::Regular).into(),
            handle: GroupHandle::Caldav,
        })
    }
}

fn opened_in_browser<'a>(
    item: GroupItem<'a, CaldavMessage>,
    web_url: Option<&str>,
) -> GroupItem<'a, CaldavMessage> {
    match web_url {
        Some(web_url) => item.on_press(
            "open the calendar in the browser",
            CaldavMessage::OpenWeb(web_url.to_owned()),
        ),
        None => item,
    }
}
