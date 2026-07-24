//! odin's `Journal`: failed systemd units (system + user scope) as a flat row of
//! count-badge indicators, each hidden while its count is zero — a healthy
//! system shows nothing. Hovering an indicator opens its timestamped
//! [`Log`](crate::data_sources::TooltipBody::Log) tooltip.
//!
//! Kernel errors (odin's third indicator) are not ported yet — they need a
//! journal reader (`/dev/kmsg` is root-only here).

use iced::{
    Alignment, Element, Subscription,
    widget::Row,
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, systemd_failed::Failed},
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::icon_gauge::source_reading,
};

#[derive(Default)]
pub struct JournalGroup {
    system: SourceCell<Failed<false>>,
    user: SourceCell<Failed<true>>,
}

#[derive(Debug, Clone)]
pub enum JournalMessage {
    System(Option<Failed<false>>),
    User(Option<Failed<true>>),
    Tooltip(TooltipMessage),
}

impl JournalGroup {
    pub fn subscription() -> Subscription<JournalMessage> {
        Subscription::batch([
            SourceCell::subscription().map(JournalMessage::System),
            SourceCell::subscription().map(JournalMessage::User),
        ])
    }

    pub fn update(&mut self, msg: JournalMessage) {
        match msg {
            JournalMessage::System(reading) => self.system.load(reading),
            JournalMessage::User(reading) => self.user.load(reading),
            JournalMessage::Tooltip(_) => {}
        }
    }

    pub fn system_cell(&self) -> &SourceCell<Failed<false>> {
        &self.system
    }

    pub fn user_cell(&self) -> &SourceCell<Failed<true>> {
        &self.user
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, JournalMessage>> {
        let indicators: Vec<Element<'a, JournalMessage>> = [
            indicator(
                config,
                self.system.get().map(|w| w as &dyn BarWidget),
                TooltipSource::JournalSystem,
            ),
            indicator(
                config,
                self.user.get().map(|w| w as &dyn BarWidget),
                TooltipSource::JournalUser,
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
    source: TooltipSource,
) -> Option<Element<'a, JournalMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    Some(hoverable(reading, source, JournalMessage::Tooltip))
}
