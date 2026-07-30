//! odin's `Journal`: failed systemd units (system + user scope) and recent
//! kernel errors as a flat row of count-badge indicators, each hidden while its
//! count is zero — a healthy system shows nothing. Hovering an indicator opens
//! its timestamped [`Log`](crate::data::source::TooltipBody::Log) tooltip.

use iced::{Alignment, Element, Subscription, widget::Row};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{BarWidget, Tooltip, kernel_errors::KernelErrors, systemd_failed::Failed},
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
            SourceCell::subscription().map(JournalMessage::System),
            SourceCell::subscription().map(JournalMessage::User),
            SourceCell::subscription().map(JournalMessage::Kernel),
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
        match target {
            HoverTarget::JournalSystem => self.system.tooltip(),
            HoverTarget::JournalUser => self.user.tooltip(),
            HoverTarget::JournalKernel => self.kernel.tooltip(),
            _ => None,
        }
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
