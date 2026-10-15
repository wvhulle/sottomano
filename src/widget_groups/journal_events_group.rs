use iced::{Subscription, Task};

mod coredump_log;
mod kernel_error_log;
mod oom_kill_log;
mod service_error_log;

use crate::{
    config::Config,
    data::source::{
        Delivery, coredumps::CoredumpEntry, kernel_errors::KernelOccurrence,
        oom_kills::OomKillEntry, service_errors::ServiceErrors,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::listening,
    widget_groups::terminal_command::TerminalCommand,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
        widget_state::WidgetState,
    },
};

pub const MESSAGE_CAP: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTarget {
    Events,
    Kernel,
    ServiceErrors,
    Coredumps,
    OomKills,
}

impl From<JournalTarget> for HoverTarget {
    fn from(target: JournalTarget) -> Self {
        HoverTarget::Journal(target)
    }
}

#[derive(Default)]
pub struct JournalEventsGroup {
    kernel: ContinuousReading<Vec<KernelOccurrence>>,
    service_errors: ContinuousReading<ServiceErrors>,
    coredumps: ContinuousReading<Vec<CoredumpEntry>>,
    oom_kills: ContinuousReading<Vec<OomKillEntry>>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum JournalEventsMessage {
    Kernel(Delivery<Vec<KernelOccurrence>>),
    ServiceErrors(Delivery<ServiceErrors>),
    Coredumps(Delivery<Vec<CoredumpEntry>>),
    OomKills(Delivery<Vec<OomKillEntry>>),
    OpenLog(TerminalCommand),
    Expansion(ExpansionMessage),
}

impl JournalEventsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            listening::<Vec<KernelOccurrence>>(()).map(JournalEventsMessage::Kernel),
            listening::<ServiceErrors>(()).map(JournalEventsMessage::ServiceErrors),
            listening::<Vec<CoredumpEntry>>(()).map(JournalEventsMessage::Coredumps),
            listening::<Vec<OomKillEntry>>(()).map(JournalEventsMessage::OomKills),
        ])
    }

    pub fn update(&mut self, msg: JournalEventsMessage) -> Task<JournalEventsMessage> {
        match msg {
            JournalEventsMessage::Kernel(delivery) => self.kernel.push_or_keep(delivery),
            JournalEventsMessage::ServiceErrors(delivery) => {
                self.service_errors.push_or_keep(delivery);
            }
            JournalEventsMessage::Coredumps(delivery) => self.coredumps.push_or_keep(delivery),
            JournalEventsMessage::OomKills(delivery) => self.oom_kills.push_or_keep(delivery),
            JournalEventsMessage::OpenLog(journal) => return journal.open(),
            JournalEventsMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: JournalTarget) -> Option<Tooltip> {
        match target {
            JournalTarget::Kernel => self.kernel.present_tooltip(),
            JournalTarget::ServiceErrors => self.service_errors.present_tooltip(),
            JournalTarget::Coredumps => self.coredumps.present_tooltip(),
            JournalTarget::OomKills => self.oom_kills.present_tooltip(),
            JournalTarget::Events => self
                .kernel
                .tooltip_with(self.tooltip())
                .or_else(|| self.service_errors.tooltip_with(self.tooltip()))
                .or_else(|| self.coredumps.tooltip_with(self.tooltip()))
                .or_else(|| self.oom_kills.tooltip_with(self.tooltip())),
        }
    }

    pub fn group_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> GroupContent<'a, JournalEventsMessage> {
        let has_kinds = self.kinds().iter().any(Option::is_some);
        GroupContent::new(
            &self.expansion,
            JournalEventsMessage::Expansion,
            [
                has_kinds.then(|| GroupItem::reading(self, JournalTarget::Events)),
                self.kernel_log_item(config),
                self.service_errors
                    .present_reading()
                    .map(|service_errors| service_errors.group_item(config)),
                self.coredump_log_item(config),
                self.oom_kill_log_item(config),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn kinds(&self) -> [Option<&dyn BarWidget>; 4] {
        [
            self.kernel.present_reading().map(|w| w as &dyn BarWidget),
            self.service_errors
                .present_reading()
                .map(|w| w as &dyn BarWidget),
            self.coredumps
                .present_reading()
                .map(|w| w as &dyn BarWidget),
            self.oom_kills
                .present_reading()
                .map(|w| w as &dyn BarWidget),
        ]
    }
}

impl BarWidget for JournalEventsGroup {
    fn name(&self) -> &'static str {
        "Journal events"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Scroll(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let total: u32 = self
            .kinds()
            .into_iter()
            .flatten()
            .filter_map(BarWidget::badge)
            .sum();
        (total > 0).then_some(total)
    }

    fn state(&self) -> WidgetState {
        self.kinds()
            .into_iter()
            .flatten()
            .map(|kind| kind.state().severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let kernel = self
            .kernel
            .present_reading()
            .map(|errors| KeyValue::new("Kernel errors", errors.len().to_string()));
        let service = self
            .service_errors
            .present_reading()
            .map(|errors| KeyValue::new("Service errors", errors.entry_count.to_string()));
        let coredumps = self
            .coredumps
            .present_reading()
            .map(|coredumps| KeyValue::new("Coredumps", coredumps.len().to_string()));
        let oom_kills = self
            .oom_kills
            .present_reading()
            .map(|kills| KeyValue::new("OOM kills", kills.len().to_string()));
        TooltipBody::sections([Section::dictionary(
            kernel
                .into_iter()
                .chain(service)
                .chain(coredumps)
                .chain(oom_kills),
        )
        .titled("Last hour")])
    }
}
