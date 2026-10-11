use iced::{Subscription, Task};

pub mod kernel_error_log;
pub mod service_error_log;

use crate::{
    config::Config,
    data::source::{kernel_errors::KernelOccurrence, service_errors::ServiceErrors},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::journal_subscription,
    widget_groups::terminal_journal::TerminalJournal,
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
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum JournalEventsMessage {
    Kernel(Vec<KernelOccurrence>),
    ServiceErrors(ServiceErrors),
    OpenLog(TerminalJournal),
    Expansion(ExpansionMessage),
}

impl JournalEventsGroup {
    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            journal_subscription(
                KernelOccurrence::merged_oldest_first,
                JournalEventsMessage::Kernel,
            ),
            journal_subscription(ServiceErrors::from, JournalEventsMessage::ServiceErrors),
        ])
    }

    pub fn update(&mut self, msg: JournalEventsMessage) -> Task<JournalEventsMessage> {
        match msg {
            JournalEventsMessage::Kernel(reading) => self.kernel.load(reading),
            JournalEventsMessage::ServiceErrors(reading) => self.service_errors.load(reading),
            JournalEventsMessage::OpenLog(journal) => return journal.open(),
            JournalEventsMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: JournalTarget) -> Option<Tooltip> {
        let body = match target {
            JournalTarget::Kernel => self.kernel.present_reading()?.tooltip(),
            JournalTarget::ServiceErrors => self.service_errors.present_reading()?.tooltip(),
            JournalTarget::Events => self.tooltip(),
        };
        Some(Tooltip::of(body))
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
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn kinds(&self) -> [Option<&dyn BarWidget>; 2] {
        [
            self.kernel.present_reading().map(|w| w as &dyn BarWidget),
            self.service_errors
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
        TooltipBody::sections(&[Section::dictionary(
            "Last hour",
            kernel.into_iter().chain(service),
        )])
    }
}
