use iced::{
    Alignment, Element, Subscription, Task,
    futures::StreamExt,
    widget::{Row, mouse_area},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        BarWidget, Tooltip, journal, kernel_errors::KernelErrors, niri, systemd_failed::Failed,
    },
    message_subscriptions::blocking,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::source_cell::SourceCell,
    widgets::{icon_gauge::source_reading, pager_slot::permanently_inactive_slot},
};

#[derive(Default)]
pub struct JournalGroup {
    system: SourceCell<Failed<false>>,
    user: SourceCell<Failed<true>>,
    kernel: SourceCell<KernelErrors>,
}

#[derive(Debug, Clone)]
pub enum JournalMessage {
    System(Option<Failed<false>>),
    User(Option<Failed<true>>),
    Kernel(Option<KernelErrors>),
    OpenLog(Vec<String>),
    Tooltip(TooltipMessage),
}

impl JournalGroup {
    pub fn subscription() -> Subscription<JournalMessage> {
        Subscription::batch([
            kernel_subscription(),
            failed_subscription::<false>("systemd-failed-system", JournalMessage::System),
            failed_subscription::<true>("systemd-failed-user", JournalMessage::User),
        ])
    }

    pub fn update(&mut self, msg: JournalMessage) -> Task<JournalMessage> {
        match msg {
            JournalMessage::System(reading) => self.system.load(reading),
            JournalMessage::User(reading) => self.user.load(reading),
            JournalMessage::Kernel(reading) => self.kernel.load(reading),
            JournalMessage::OpenLog(command) => {
                return blocking("journal log terminal", move || niri::spawn(command)).discard();
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
        Some(Tooltip { body, footer: None })
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
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
                .spacing(config.layout.horizontal_widget_spacing_px)
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

fn failed_subscription<const USER: bool>(
    key: &'static str,
    map: fn(Option<Failed<USER>>) -> JournalMessage,
) -> Subscription<JournalMessage> {
    Subscription::run_with((key, map), |&(_, map)| {
        journal::failed_stream::<USER>().map(move |units| map(Failed::from_units(units)))
    })
}

fn indicator<'a>(
    config: &ResolvedConfig,
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
    Some(mouse_area(hover).on_press(open_log).into())
}
