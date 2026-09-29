use iced::{Subscription, Task, futures::StreamExt};
use tokio_stream::wrappers::ReceiverStream;

pub mod failed_unit_log;
pub mod kernel_error_log;

use crate::{
    config::Config,
    data::source::{
        journal,
        kernel_errors::KernelErrors,
        niri,
        systemd_failed::{Failed, JournalScope, System, User},
    },
    message_subscriptions::blocking,
    wayland_layers::tooltip::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Tooltip, TooltipBody, row},
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Default)]
pub struct JournalGroup {
    system: LastPoll<Failed<System>>,
    user: LastPoll<Failed<User>>,
    kernel: LastPoll<KernelErrors>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum JournalMessage {
    System(Option<Failed<System>>),
    User(Option<Failed<User>>),
    Kernel(Option<KernelErrors>),
    OpenLog(Vec<String>),
    Expansion(ExpansionMessage),
}

impl JournalGroup {
    pub fn subscription() -> Subscription<JournalMessage> {
        Subscription::batch([
            kernel_subscription(),
            failed_subscription::<System>("systemd-failed-system", JournalMessage::System),
            failed_subscription::<User>("systemd-failed-user", JournalMessage::User),
        ])
    }

    pub fn update(&mut self, msg: JournalMessage) -> Task<JournalMessage> {
        match msg {
            JournalMessage::System(reading) => self.system.load(reading),
            JournalMessage::User(reading) => self.user.load(reading),
            JournalMessage::Kernel(reading) => self.kernel.load(reading),
            JournalMessage::OpenLog(command) => {
                return blocking("journal log terminal", move || {
                    niri::send_action(&niri_ipc::Action::Spawn { command });
                })
                .discard();
            }
            JournalMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::JournalSystem => self.system.get()?.tooltip(),
            HoverTarget::JournalUser => self.user.get()?.tooltip(),
            HoverTarget::JournalKernel => self.kernel.get()?.tooltip(),
            HoverTarget::Journal => self.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, JournalMessage> {
        let summary = self
            .scopes()
            .iter()
            .any(Option::is_some)
            .then_some(self as &dyn BarWidget);
        WidgetGroup::new(&self.expansion, JournalMessage::Expansion)
            .reading(summary, HoverTarget::Journal)
            .item(self.system.get().map(|system| {
                log_reading(
                    config,
                    system,
                    HoverTarget::JournalSystem,
                    system.follow_log_args(),
                )
            }))
            .item(self.user.get().map(|user| {
                log_reading(
                    config,
                    user,
                    HoverTarget::JournalUser,
                    user.follow_log_args(),
                )
            }))
            .item(self.kernel.get().map(|kernel| {
                log_reading(
                    config,
                    kernel,
                    HoverTarget::JournalKernel,
                    KernelErrors::follow_log_args(),
                )
            }))
    }

    fn scopes(&self) -> [Option<&dyn BarWidget>; 3] {
        [
            self.system.get().map(|w| w as &dyn BarWidget),
            self.user.get().map(|w| w as &dyn BarWidget),
            self.kernel.get().map(|w| w as &dyn BarWidget),
        ]
    }
}

impl BarWidget for JournalGroup {
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
                self.system
                    .get()
                    .map(|system| row("Failed system units", scope_count_text(system))),
                self.user
                    .get()
                    .map(|user| row("Failed user units", scope_count_text(user))),
                self.kernel
                    .get()
                    .map(|kernel| row("Kernel errors", scope_count_text(kernel))),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }
}

fn kernel_subscription() -> Subscription<JournalMessage> {
    Subscription::run(|| {
        ReceiverStream::new(journal::kernel_errors())
            .map(|entries| JournalMessage::Kernel(KernelErrors::from_entries(entries)))
    })
}

fn failed_subscription<S: JournalScope + 'static>(
    key: &'static str,
    map: fn(Option<Failed<S>>) -> JournalMessage,
) -> Subscription<JournalMessage> {
    Subscription::run_with((key, map), |&(_, map)| {
        journal::failed_stream(S::PER_USER).map(move |units| map(Failed::from_units(units)))
    })
}

fn log_reading<'a>(
    config: &Config,
    widget: &'a dyn BarWidget,
    target: HoverTarget,
    journalctl_args: Vec<String>,
) -> GroupItem<'a, JournalMessage> {
    GroupItem::reading(widget, target).on_press(JournalMessage::OpenLog(
        config
            .terminal_command_prefix
            .iter()
            .cloned()
            .chain(["journalctl".to_string()])
            .chain(journalctl_args)
            .collect(),
    ))
}

fn scope_count_text(scope: &dyn BarWidget) -> String {
    scope.badge().unwrap_or(0).to_string()
}
