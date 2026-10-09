use iced::{Subscription, Task, futures::StreamExt};

pub mod failed_unit_log;

use crate::{
    config::Config,
    data::source::systemd_units::{Failed, System, UnitScope, User, failed_stream},
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::terminal_journal::TerminalJournal,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Default)]
pub struct SystemdUnitsGroup {
    system: LastPoll<Failed<System>>,
    user: LastPoll<Failed<User>>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum SystemdUnitsMessage {
    System(Option<Failed<System>>),
    User(Option<Failed<User>>),
    OpenLog(TerminalJournal),
    Expansion(ExpansionMessage),
}

impl SystemdUnitsGroup {
    pub fn subscription() -> Subscription<SystemdUnitsMessage> {
        Subscription::batch([
            failed_subscription::<System>("systemd-failed-system", SystemdUnitsMessage::System),
            failed_subscription::<User>("systemd-failed-user", SystemdUnitsMessage::User),
        ])
    }

    pub fn update(&mut self, msg: SystemdUnitsMessage) -> Task<SystemdUnitsMessage> {
        match msg {
            SystemdUnitsMessage::System(reading) => self.system.load(reading),
            SystemdUnitsMessage::User(reading) => self.user.load(reading),
            SystemdUnitsMessage::OpenLog(journal) => return journal.open(),
            SystemdUnitsMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::SystemdUnitsSystem => self.system.get()?.tooltip(),
            HoverTarget::SystemdUnitsUser => self.user.get()?.tooltip(),
            HoverTarget::SystemdUnits => self.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, SystemdUnitsMessage> {
        let summary = self
            .scopes()
            .iter()
            .any(Option::is_some)
            .then_some(self as &dyn BarWidget);
        let open_log =
            |args| SystemdUnitsMessage::OpenLog(TerminalJournal::following(config, args));
        WidgetGroup::new(&self.expansion, SystemdUnitsMessage::Expansion)
            .reading(summary, HoverTarget::SystemdUnits)
            .item(self.system.get().map(|system| system.group_item(config)))
            .item(self.user.get().map(|user| user.group_item(config)))
            .standalone_alerts(
                self.system
                    .get()
                    .into_iter()
                    .flat_map(|system| system.unit_alerts(open_log, config.locale)),
            )
            .standalone_alerts(
                self.user
                    .get()
                    .into_iter()
                    .flat_map(|user| user.unit_alerts(open_log, config.locale)),
            )
    }

    fn scopes(&self) -> [Option<&dyn BarWidget>; 2] {
        [
            self.system.get().map(|w| w as &dyn BarWidget),
            self.user.get().map(|w| w as &dyn BarWidget),
        ]
    }
}

impl BarWidget for SystemdUnitsGroup {
    fn icon(&self) -> Glyph {
        Phosphor::Heartbeat(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let total: u32 = self
            .scopes()
            .into_iter()
            .flatten()
            .filter_map(BarWidget::badge)
            .sum();
        (total > 0).then_some(total)
    }

    fn state(&self) -> WidgetState {
        self.scopes()
            .into_iter()
            .flatten()
            .map(|scope| scope.state().severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(
            [
                self.system.get().map(Failed::count_row),
                self.user.get().map(Failed::count_row),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }
}

fn failed_subscription<S: UnitScope + 'static>(
    key: &'static str,
    map: fn(Option<Failed<S>>) -> SystemdUnitsMessage,
) -> Subscription<SystemdUnitsMessage> {
    Subscription::run_with((key, map), |&(_, map)| {
        failed_stream(S::PER_USER).map(move |units| map(Failed::from_units(units)))
    })
}
