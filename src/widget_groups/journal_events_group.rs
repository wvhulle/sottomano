use iced::{Subscription, Task};

pub mod kernel_error_log;
pub mod service_error_log;

use crate::{
    config::Config,
    data::source::{kernel_errors::KernelErrors, service_errors::ServiceErrors},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::journal_subscription,
    widget_groups::terminal_journal::TerminalJournal,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
        widget_state::WidgetState,
    },
};

pub const MESSAGE_CAP: usize = 100;

#[derive(Default)]
pub struct JournalEventsGroup {
    kernel: LastPoll<KernelErrors>,
    service_errors: LastPoll<ServiceErrors>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum JournalEventsMessage {
    Kernel(Option<KernelErrors>),
    ServiceErrors(Option<ServiceErrors>),
    OpenLog(TerminalJournal),
    Expansion(ExpansionMessage),
}

impl JournalEventsGroup {
    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            journal_subscription(KernelErrors::from_entries, JournalEventsMessage::Kernel),
            journal_subscription(
                ServiceErrors::from_entries,
                JournalEventsMessage::ServiceErrors,
            ),
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::JournalKernel => self.kernel.get()?.tooltip(),
            HoverTarget::JournalServiceErrors => self.service_errors.get()?.tooltip(),
            HoverTarget::JournalEvents => self.tooltip(),
            _ => return None,
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
                has_kinds.then(|| GroupItem::reading(self, HoverTarget::JournalEvents)),
                self.kernel.get().map(|kernel| kernel.group_item(config)),
                self.service_errors
                    .get()
                    .map(|service_errors| service_errors.group_item(config)),
            ]
            .into_iter()
            .flatten(),
        )
    }

    fn kinds(&self) -> [Option<&dyn BarWidget>; 2] {
        [
            self.kernel.get().map(|w| w as &dyn BarWidget),
            self.service_errors.get().map(|w| w as &dyn BarWidget),
        ]
    }
}

impl BarWidget for JournalEventsGroup {
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
            .get()
            .map(|errors| KeyValue::new("Kernel errors", errors.oldest_first.len().to_string()));
        let service = self
            .service_errors
            .get()
            .map(|errors| KeyValue::new("Service errors", errors.entry_count.to_string()));
        TooltipBody::sections(&[Section::dictionary(
            "Last hour",
            kernel.into_iter().chain(service),
        )])
    }
}
