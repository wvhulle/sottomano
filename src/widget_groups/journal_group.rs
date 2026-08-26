use iced::{
    Alignment, Element, Subscription, Task,
    futures::StreamExt,
    widget::{Row, button},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    config::Config,
    data::{
        last_poll::LastPoll,
        source::{
            BarWidget, Tooltip, journal,
            kernel_errors::KernelErrors,
            niri,
            systemd_failed::{Failed, JournalScope, System, User},
        },
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{flat_button, pager_slot::permanently_inactive_slot, source_reading::source_reading},
};

#[derive(Default)]
pub struct JournalGroup {
    system: LastPoll<Failed<System>>,
    user: LastPoll<Failed<User>>,
    kernel: LastPoll<KernelErrors>,
}

#[derive(Debug, Clone)]
pub enum JournalMessage {
    System(Option<Failed<System>>),
    User(Option<Failed<User>>),
    Kernel(Option<KernelErrors>),
    OpenLog(Vec<String>),
    Tooltip(TooltipMessage),
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
                return blocking("journal log terminal", move || niri::spawn(&command)).discard();
            }
            JournalMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::JournalSystem => self.system.get()?.tooltip(),
            HoverTarget::JournalUser => self.user.get()?.tooltip(),
            HoverTarget::JournalKernel => self.kernel.get()?.tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, JournalMessage>> {
        let indicators: Vec<Element<'a, JournalMessage>> = [
            indicator(
                config,
                self.system.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalSystem,
                self.system.get().map(Failed::follow_log_args),
            ),
            indicator(
                config,
                self.user.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalUser,
                self.user.get().map(Failed::follow_log_args),
            ),
            indicator(
                config,
                self.kernel.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalKernel,
                self.kernel.get().map(|_| KernelErrors::follow_log_args()),
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        if indicators.is_empty() {
            return None;
        }
        Some(permanently_inactive_slot(
            Row::with_children(indicators)
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
            config,
            position,
        ))
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

fn indicator<'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
    target: HoverTarget,
    journalctl_args: Option<Vec<String>>,
) -> Option<Element<'a, JournalMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    let hover = hoverable(reading, target, JournalMessage::Tooltip);
    let open_log = JournalMessage::OpenLog(
        config
            .terminal_command_prefix
            .iter()
            .cloned()
            .chain(["journalctl".to_string()])
            .chain(journalctl_args?)
            .collect(),
    );
    Some(
        button(hover)
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(open_log)
            .into(),
    )
}
