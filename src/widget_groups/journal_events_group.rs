use iced::{Subscription, Task};

pub mod kernel_error_log;
pub mod service_error_log;

use crate::{
    config::Config,
    data::source::{kernel_errors::KernelErrors, service_errors::ServiceErrors},
    desktop_shell::{hover_tooltips::HoverTarget, monitor_surfaces::Position},
    message_subscriptions::journal_subscription,
    widget_groups::terminal_journal::TerminalJournal,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::{GroupItem, Press},
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
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

    pub fn widget_group<'a, Outer: 'a>(
        &'a self,
        config: &'a Config,
        position: Position,
        lift: fn(JournalEventsMessage) -> Outer,
    ) -> WidgetGroup<'a, Outer> {
        let has_kinds = self.kinds().iter().any(Option::is_some);
        let opens_log = |widget: &'a dyn BarWidget, target, journalctl_args| GroupItem {
            on_press: Some(Press {
                does: TerminalJournal::PRESS_HINT,
                message: JournalEventsMessage::OpenLog(TerminalJournal::following(
                    config,
                    journalctl_args,
                )),
            }),
            ..GroupItem::reading(widget, target)
        };
        WidgetGroup::new(
            &self.expansion,
            JournalEventsMessage::Expansion,
            GroupHandle {
                face: HandleFace::FirstItem,
                preview: None,
                forwards_scroll: false,
            },
            &[
                has_kinds.then(|| GroupItem::reading(self, HoverTarget::JournalEvents)),
                self.kernel.get().map(|kernel| {
                    opens_log(
                        kernel,
                        HoverTarget::JournalKernel,
                        KernelErrors::follow_log_args(),
                    )
                }),
                self.service_errors.get().map(|service_errors| {
                    opens_log(
                        service_errors,
                        HoverTarget::JournalServiceErrors,
                        service_errors.follow_log_args(),
                    )
                }),
            ],
            config,
            position,
            lift,
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
        TooltipBody::sections(&[Section::titled(
            "Last hour",
            &[
                (
                    "Kernel errors",
                    self.kernel
                        .get()
                        .map(|errors| errors.oldest_first.len().to_string()),
                ),
                (
                    "Service errors",
                    self.service_errors
                        .get()
                        .map(|errors| errors.entry_count.to_string()),
                ),
            ],
        )])
    }
}
