use iced::{Subscription, Task, futures::StreamExt};
use tokio_stream::wrappers::ReceiverStream;

pub mod coredump_log;
pub mod kernel_error_log;
pub mod oom_kill_log;
pub mod service_error_log;

use crate::{
    config::Config,
    data::source::{
        coredumps::Coredumps,
        journal::{JournalEvent, journal_events},
        kernel_errors::KernelErrors,
        oom_kills::OomKills,
        service_errors::ServiceErrors,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    widget_groups::terminal_journal::TerminalJournal,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Tooltip, TooltipBody, section},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

pub const MESSAGE_CAP: usize = 100;

#[derive(Default)]
pub struct JournalEventsGroup {
    kernel: LastPoll<KernelErrors>,
    coredumps: LastPoll<Coredumps>,
    oom_kills: LastPoll<OomKills>,
    service_errors: LastPoll<ServiceErrors>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum JournalEventsMessage {
    Kernel(Option<KernelErrors>),
    Coredumps(Option<Coredumps>),
    OomKills(Option<OomKills>),
    ServiceErrors(Option<ServiceErrors>),
    OpenLog(TerminalJournal),
    Expansion(ExpansionMessage),
}

impl JournalEventsGroup {
    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            events_subscription(KernelErrors::from_entries, JournalEventsMessage::Kernel),
            events_subscription(Coredumps::from_entries, JournalEventsMessage::Coredumps),
            events_subscription(OomKills::from_entries, JournalEventsMessage::OomKills),
            events_subscription(
                ServiceErrors::from_entries,
                JournalEventsMessage::ServiceErrors,
            ),
        ])
    }

    pub fn update(&mut self, msg: JournalEventsMessage) -> Task<JournalEventsMessage> {
        match msg {
            JournalEventsMessage::Kernel(reading) => self.kernel.load(reading),
            JournalEventsMessage::Coredumps(reading) => self.coredumps.load(reading),
            JournalEventsMessage::OomKills(reading) => self.oom_kills.load(reading),
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
            HoverTarget::JournalCoredumps => self.coredumps.get()?.tooltip(),
            HoverTarget::JournalOomKills => self.oom_kills.get()?.tooltip(),
            HoverTarget::JournalServiceErrors => self.service_errors.get()?.tooltip(),
            HoverTarget::JournalEvents => self.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, JournalEventsMessage> {
        let summary = self
            .kinds()
            .iter()
            .any(Option::is_some)
            .then_some(self as &dyn BarWidget);
        let opens_log = |widget: &'a dyn BarWidget, target, journalctl_args| {
            GroupItem::reading(widget, target).on_press(JournalEventsMessage::OpenLog(
                TerminalJournal::following(config, journalctl_args),
            ))
        };
        WidgetGroup::new(&self.expansion, JournalEventsMessage::Expansion)
            .reading(summary, HoverTarget::JournalEvents)
            .item(self.kernel.get().map(|kernel| {
                opens_log(
                    kernel,
                    HoverTarget::JournalKernel,
                    KernelErrors::follow_log_args(),
                )
            }))
            .item(self.coredumps.get().map(|coredumps| {
                opens_log(
                    coredumps,
                    HoverTarget::JournalCoredumps,
                    Coredumps::follow_log_args(),
                )
            }))
            .item(self.oom_kills.get().map(|oom_kills| {
                opens_log(
                    oom_kills,
                    HoverTarget::JournalOomKills,
                    OomKills::follow_log_args(),
                )
            }))
            .item(self.service_errors.get().map(|service_errors| {
                opens_log(
                    service_errors,
                    HoverTarget::JournalServiceErrors,
                    service_errors.follow_log_args(),
                )
            }))
    }

    fn kinds(&self) -> [Option<&dyn BarWidget>; 4] {
        [
            self.kernel.get().map(|w| w as &dyn BarWidget),
            self.coredumps.get().map(|w| w as &dyn BarWidget),
            self.oom_kills.get().map(|w| w as &dyn BarWidget),
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
        TooltipBody::Sections(vec![section(
            "Last hour",
            [
                self.kernel.get().map(KernelErrors::count_row),
                self.coredumps.get().map(Coredumps::count_row),
                self.oom_kills.get().map(OomKills::count_row),
                self.service_errors.get().map(ServiceErrors::count_row),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )])
    }
}

fn events_subscription<E: JournalEvent, R: 'static>(
    from_entries: fn(Vec<E>) -> Option<R>,
    map: fn(Option<R>) -> JournalEventsMessage,
) -> Subscription<JournalEventsMessage> {
    Subscription::run_with((E::NAME, from_entries, map), |&(_, from_entries, map)| {
        ReceiverStream::new(journal_events::<E>()).map(move |entries| map(from_entries(entries)))
    })
}
