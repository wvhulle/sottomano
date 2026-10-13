use std::{fmt::Display, time::SystemTime};

use crate::{
    data::source::{Delivery, LiveSource, PolledSource, Source},
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        tooltip_body::{
            Freshness, KeyValue, PollInfo, Readiness, Section, SourcedSection, Tooltip, TooltipBody,
        },
        widget_state::WidgetState,
    },
};

pub struct FailedReading {
    icon: Glyph,
    message: String,
}

impl BarWidget for FailedReading {
    fn name(&self) -> &'static str {
        "Failed reading"
    }

    fn icon(&self) -> Glyph {
        self.icon
    }

    fn state(&self) -> WidgetState {
        WidgetState::Failed
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("Error", self.message.clone())])
    }
}

pub struct ContinuousReading<S> {
    data: Option<S>,
    failure: Option<FailedReading>,
    freshness: Option<Freshness>,
}

impl<S> Default for ContinuousReading<S> {
    fn default() -> Self {
        Self {
            data: None,
            failure: None,
            freshness: None,
        }
    }
}

impl<S> ContinuousReading<S> {
    pub fn get(&self) -> Option<&S> {
        self.data.as_ref()
    }

    pub fn assume(&mut self, data: S) {
        if self.freshness.is_some() {
            self.data = Some(data);
            self.failure = None;
        }
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        let tooltip = Tooltip::sourced(body, self.freshness?);
        (!tooltip.is_empty()).then_some(tooltip)
    }

    pub fn sections(&self, sections: impl FnOnce(&S) -> Vec<Section>) -> Vec<SourcedSection> {
        let Some(freshness) = self.freshness else {
            return Vec::new();
        };
        self.data
            .as_ref()
            .map(sections)
            .unwrap_or_default()
            .into_iter()
            .map(|section| SourcedSection::new(section, Readiness::Settled, freshness))
            .collect()
    }

    fn keep<E: Display>(&mut self, reading: Result<S, E>, source: &'static str) {
        match reading {
            Ok(data) => self.data = Some(data),
            Err(failure) => tracing::warn!("{source}: {failure}"),
        }
    }
}

impl<S: Source> ContinuousReading<S> {
    fn stamp(
        &mut self,
        delivery: Delivery<S>,
        freshness: Freshness,
    ) -> Option<Result<S, S::Error>> {
        match delivery {
            Delivery::Absent => {
                *self = Self::default();
                None
            }
            Delivery::Reading(reading) => {
                self.freshness = Some(freshness);
                Some(reading)
            }
        }
    }
}

impl<S: LiveSource> ContinuousReading<S> {
    pub fn load(&mut self, data: S) {
        self.data = Some(data);
        self.failure = None;
        self.freshness = Some(Freshness::Live);
    }

    pub fn push_or_keep(&mut self, delivery: Delivery<S>) {
        if let Some(reading) = self.stamp(delivery, Freshness::Live) {
            self.keep(reading, S::NAME);
        }
    }
}

impl<S: LiveSource + BarWidget> ContinuousReading<S> {
    pub fn push(&mut self, delivery: Delivery<S>) {
        if let Some(reading) = self.stamp(delivery, Freshness::Live) {
            self.accept(reading);
        }
    }
}

impl<S: PolledSource> ContinuousReading<S> {
    fn polled_now() -> Freshness {
        Freshness::Polled(PollInfo {
            source: S::NAME,
            every: S::INTERVAL,
            last: SystemTime::now(),
        })
    }

    pub fn receive_or_keep(&mut self, delivery: Delivery<S>) {
        if let Some(reading) = self.stamp(delivery, Self::polled_now()) {
            self.keep(reading, S::NAME);
        }
    }
}

impl<S: PolledSource + BarWidget> ContinuousReading<S> {
    pub fn receive(&mut self, delivery: Delivery<S>) {
        if let Some(reading) = self.stamp(delivery, Self::polled_now()) {
            self.accept(reading);
        }
    }
}

impl<S: BarWidget> ContinuousReading<S> {
    pub fn display(&self) -> Option<&dyn BarWidget> {
        match (self.data.as_ref(), self.failure.as_ref()) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }

    pub fn failure(&self) -> Option<&dyn BarWidget> {
        self.failure
            .as_ref()
            .map(|failure| failure as &dyn BarWidget)
    }

    pub fn group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>> {
        self.display()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.display()?.tooltip())
    }

    pub fn tooltip_with_sections(
        &self,
        extra: impl IntoIterator<Item = SourcedSection>,
    ) -> Option<Tooltip> {
        let own = self.tooltip_with(self.display()?.tooltip())?;
        Some(own.with_sections(extra))
    }

    fn accept<E: Display>(&mut self, reading: Result<S, E>) {
        match reading {
            Ok(data) => {
                self.data = Some(data);
                self.failure = None;
            }
            Err(failure) => {
                let icon = self
                    .data
                    .as_ref()
                    .map(BarWidget::icon)
                    .or_else(|| self.failure.as_ref().map(|failed| failed.icon))
                    .unwrap_or_else(S::fallback_icon);
                self.data = None;
                self.failure = Some(FailedReading {
                    icon,
                    message: failure.to_string(),
                });
            }
        }
    }
}

impl<S: Occasional> ContinuousReading<S> {
    pub fn present_reading(&self) -> Option<&S> {
        self.data.as_ref().filter(|data| data.present())
    }

    pub fn present(&self) -> Option<&dyn BarWidget> {
        match (self.present_reading(), self.failure.as_ref()) {
            (Some(data), _) => Some(data),
            (None, Some(failure)) => Some(failure),
            (None, None) => None,
        }
    }

    pub fn present_group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>> {
        self.present()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn present_tooltip(&self) -> Option<Tooltip> {
        self.tooltip_with(self.present_reading()?.tooltip())
    }
}
