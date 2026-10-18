use iced::{Subscription, Task};

mod coredump_log;
mod kernel_error_log;
mod oom_kill_log;
mod service_error_log;

use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery, coredumps::CoredumpEntry, kernel_errors::KernelOccurrence,
    oom_kills::OomKillEntry, service_errors::ServiceErrors,
};

use crate::{
    background_work::source_subscription,
    hover_tooltips::HoverTarget,
    widget_groups::terminal_command::TerminalCommand,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::{ConfiguredGroupItem, GroupItem},
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
    Kernel(SourceDelivery<Vec<KernelOccurrence>>),
    ServiceErrors(SourceDelivery<ServiceErrors>),
    Coredumps(SourceDelivery<Vec<CoredumpEntry>>),
    OomKills(SourceDelivery<Vec<OomKillEntry>>),
    OpenLog(TerminalCommand),
    Expansion(ExpansionMessage),
}

impl JournalEventsGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            source_subscription::<Vec<KernelOccurrence>>(()).map(JournalEventsMessage::Kernel),
            source_subscription::<ServiceErrors>(()).map(JournalEventsMessage::ServiceErrors),
            source_subscription::<Vec<CoredumpEntry>>(()).map(JournalEventsMessage::Coredumps),
            source_subscription::<Vec<OomKillEntry>>(()).map(JournalEventsMessage::OomKills),
        ])
    }

    pub fn update(&mut self, msg: JournalEventsMessage) -> Task<JournalEventsMessage> {
        match msg {
            JournalEventsMessage::Kernel(delivery) => {
                self.kernel.receive_delivery(delivery);
            }
            JournalEventsMessage::ServiceErrors(delivery) => {
                self.service_errors.receive_delivery(delivery);
            }
            JournalEventsMessage::Coredumps(delivery) => {
                self.coredumps.receive_delivery(delivery);
            }
            JournalEventsMessage::OomKills(delivery) => {
                self.oom_kills.receive_delivery(delivery);
            }
            JournalEventsMessage::OpenLog(journal) => return journal.open(),
            JournalEventsMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: JournalTarget) -> Option<Tooltip> {
        match target {
            JournalTarget::Kernel => self.kernel.tooltip(),
            JournalTarget::ServiceErrors => self.service_errors.tooltip(),
            JournalTarget::Coredumps => self.coredumps.tooltip(),
            JournalTarget::OomKills => self.oom_kills.tooltip(),
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
                    .visible_reading()
                    .map(|service_errors| service_errors.group_item(config))
                    .or_else(|| {
                        self.service_errors.failure().map(|failure| {
                            GroupItem::reading(failure, JournalTarget::ServiceErrors)
                        })
                    }),
                self.coredump_log_item(config),
                self.oom_kill_log_item(config),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn kinds(&self) -> [Option<&dyn BarWidget>; 4] {
        [
            self.kernel.display(),
            self.service_errors.display(),
            self.coredumps.display(),
            self.oom_kills.display(),
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
            .visible_reading()
            .map(|errors| KeyValue::new("Kernel errors", errors.len().to_string()));
        let service = self
            .service_errors
            .visible_reading()
            .map(|errors| KeyValue::new("Service errors", errors.entry_count.to_string()));
        let coredumps = self
            .coredumps
            .visible_reading()
            .map(|coredumps| KeyValue::new("Coredumps", coredumps.len().to_string()));
        let oom_kills = self
            .oom_kills
            .visible_reading()
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
