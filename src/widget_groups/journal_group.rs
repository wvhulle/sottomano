use iced::{Alignment, Element, Subscription, futures::StreamExt, widget::Row};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        BarWidget, Tooltip, journal, kernel_errors::KernelErrors, systemd_failed::Failed,
    },
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widget_groups::source_cell::SourceCell,
    widgets::icon_gauge::source_reading,
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

    pub fn update(&mut self, msg: JournalMessage) {
        match msg {
            JournalMessage::System(reading) => self.system.load(reading),
            JournalMessage::User(reading) => self.user.load(reading),
            JournalMessage::Kernel(reading) => self.kernel.load(reading),
            JournalMessage::Tooltip(_) => {}
        }
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

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, JournalMessage>> {
        let indicators: Vec<Element<'a, JournalMessage>> = [
            indicator(
                config,
                self.system.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalSystem,
            ),
            indicator(
                config,
                self.user.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalUser,
            ),
            indicator(
                config,
                self.kernel.get().map(|w| w as &dyn BarWidget),
                HoverTarget::JournalKernel,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        if indicators.is_empty() {
            return None;
        }
        Some(
            Row::with_children(indicators)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
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

/// One scope's indicator (icon + count badge), hoverable for its log tooltip;
/// `None` when that scope has no failed units.
fn indicator<'a>(
    config: &ResolvedConfig,
    widget: Option<&dyn BarWidget>,
    target: HoverTarget,
) -> Option<Element<'a, JournalMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    Some(hoverable(reading, target, JournalMessage::Tooltip))
}
