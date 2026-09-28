use iced::{
    Alignment, Element, Subscription, Task,
    futures::StreamExt,
    widget::{Row, button},
};
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
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Level},
        flat_button,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        phosphor,
        source_reading::source_reading,
        tooltip_body::{Tooltip, TooltipBody},
    },
};

#[derive(Default)]
pub struct JournalGroup {
    system: LastPoll<Failed<System>>,
    user: LastPoll<Failed<User>>,
    kernel: LastPoll<KernelErrors>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum JournalMessage {
    System(Option<Failed<System>>),
    User(Option<Failed<User>>),
    Kernel(Option<KernelErrors>),
    OpenLog(Vec<String>),
    Menu(MenuMessage),
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
                return blocking("journal log terminal", move || {
                    niri::send_action(&niri_ipc::Action::Spawn { command });
                })
                .discard();
            }
            JournalMessage::Menu(msg) => {
                self.menu.update(msg);
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
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, JournalMessage>> {
        if self.indicators(config, 1.0).is_empty() {
            return None;
        }
        let content = move |reveal: f32| {
            Row::with_children(self.indicators(config, reveal))
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        let face =
            move || source_reading(config, Some(self), 1.0).unwrap_or_else(|| Row::new().into());
        Some(
            self.menu
                .slot(face, content, JournalMessage::Menu)
                .view(config, position),
        )
    }

    fn indicators<'a>(&self, config: &Config, opacity: f32) -> Vec<Element<'a, JournalMessage>> {
        [
            indicator(
                config,
                opacity,
                self.system.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalSystem,
                self.system.get().map(Failed::follow_log_args),
            ),
            indicator(
                config,
                opacity,
                self.user.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalUser,
                self.user.get().map(Failed::follow_log_args),
            ),
            indicator(
                config,
                opacity,
                self.kernel.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalKernel,
                self.kernel.get().map(|_| KernelErrors::follow_log_args()),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
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
        phosphor::Heartbeat::Regular.into()
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

    fn level(&self) -> Level {
        self.scopes()
            .into_iter()
            .flatten()
            .map(BarWidget::level)
            .max()
            .unwrap_or_default()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
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
    opacity: f32,
    widget: Option<&dyn BarWidget>,
    target: HoverTarget,
    journalctl_args: Option<Vec<String>>,
) -> Option<Element<'a, JournalMessage>> {
    let reading = source_reading(config, widget, opacity)?;
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
