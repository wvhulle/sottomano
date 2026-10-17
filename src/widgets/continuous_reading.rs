use std::{mem::take, time::SystemTime};

use crate::{
    data::source::{Delivery, LiveSource, PolledSource, Source},
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        group_item::GroupItem,
        tooltip_body::{Freshness, Readiness, Section, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct FailedReading<S: Source> {
    icon: Glyph,
    error: S::Error,
}

impl<S: Source> BarWidget for FailedReading<S> {
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
        TooltipBody::sections([self.error.clone().into()])
    }
}

#[derive(Default)]
pub enum ContinuousReading<S: Source> {
    #[default]
    Absent,
    Awaiting {
        freshness: Freshness,
    },
    Read {
        data: S,
        freshness: Freshness,
    },
    Failed {
        failure: FailedReading<S>,
        freshness: Freshness,
    },
}

impl<S: Source> ContinuousReading<S> {
    pub fn get(&self) -> Option<&S> {
        match self {
            ContinuousReading::Read { data, .. } => Some(data),
            ContinuousReading::Absent
            | ContinuousReading::Awaiting { .. }
            | ContinuousReading::Failed { .. } => None,
        }
    }

    fn freshness(&self) -> Option<Freshness> {
        match self {
            ContinuousReading::Absent => None,
            ContinuousReading::Awaiting { freshness }
            | ContinuousReading::Read { freshness, .. }
            | ContinuousReading::Failed { freshness, .. } => Some(*freshness),
        }
    }

    pub fn assume(&mut self, data: S) {
        if let Some(freshness) = self.freshness() {
            *self = ContinuousReading::Read { data, freshness };
        }
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        Some(Tooltip::sourced(body, self.freshness()?))
    }

    pub fn sections(&self, sections: impl FnOnce(&S) -> Vec<Section>) -> Vec<Section> {
        let ContinuousReading::Read { data, freshness } = self else {
            return Vec::new();
        };
        sections(data)
            .into_iter()
            .map(|section| section.stamped(Readiness::Settled, *freshness))
            .collect()
    }

    fn kept(self, delivery: Delivery<S>, freshness: Freshness) -> Self {
        let Delivery::Reading(reading) = delivery else {
            return ContinuousReading::Absent;
        };
        match reading {
            Ok(data) => ContinuousReading::Read { data, freshness },
            Err(error) => {
                tracing::warn!("{}: {error}", S::NAME);
                self.restamped(freshness)
            }
        }
    }

    fn restamped(self, freshness: Freshness) -> Self {
        match self {
            ContinuousReading::Absent | ContinuousReading::Awaiting { .. } => {
                ContinuousReading::Awaiting { freshness }
            }
            ContinuousReading::Read { data, .. } => ContinuousReading::Read { data, freshness },
            ContinuousReading::Failed { failure, .. } => {
                ContinuousReading::Failed { failure, freshness }
            }
        }
    }

    fn keep_with(&mut self, delivery: Delivery<S>, freshness: Freshness) {
        *self = take(self).kept(delivery, freshness);
    }
}

impl<S: Source + BarWidget> ContinuousReading<S> {
    fn accepted(self, delivery: Delivery<S>, freshness: Freshness) -> Self {
        let Delivery::Reading(reading) = delivery else {
            return ContinuousReading::Absent;
        };
        match reading {
            Ok(data) => ContinuousReading::Read { data, freshness },
            Err(error) => ContinuousReading::Failed {
                failure: FailedReading {
                    icon: self.last_icon(),
                    error,
                },
                freshness,
            },
        }
    }

    fn last_icon(&self) -> Glyph {
        match self {
            ContinuousReading::Read { data, .. } => data.icon(),
            ContinuousReading::Failed { failure, .. } => failure.icon,
            ContinuousReading::Absent | ContinuousReading::Awaiting { .. } => S::fallback_icon(),
        }
    }

    fn accept_with(&mut self, delivery: Delivery<S>, freshness: Freshness) {
        *self = take(self).accepted(delivery, freshness);
    }
}

impl<S: LiveSource> ContinuousReading<S> {
    pub fn load(&mut self, data: S) {
        *self = ContinuousReading::Read {
            data,
            freshness: Freshness::Live,
        };
    }

    pub fn push_or_keep(&mut self, delivery: Delivery<S>) {
        self.keep_with(delivery, Freshness::Live);
    }
}

impl<S: LiveSource + BarWidget> ContinuousReading<S> {
    pub fn push(&mut self, delivery: Delivery<S>) {
        self.accept_with(delivery, Freshness::Live);
    }
}

impl<S: PolledSource> ContinuousReading<S> {
    fn polled_now() -> Freshness {
        Freshness::Polled {
            source: S::NAME,
            every: S::INTERVAL,
            last: SystemTime::now(),
        }
    }

    pub fn receive_or_keep(&mut self, delivery: Delivery<S>) {
        self.keep_with(delivery, Self::polled_now());
    }
}

impl<S: PolledSource + BarWidget> ContinuousReading<S> {
    pub fn receive(&mut self, delivery: Delivery<S>) {
        self.accept_with(delivery, Self::polled_now());
    }
}

impl<S: Source + BarWidget> ContinuousReading<S> {
    pub fn display(&self) -> Option<&dyn BarWidget> {
        match self {
            ContinuousReading::Read { data, .. } => Some(data),
            ContinuousReading::Failed { failure, .. } => Some(failure),
            ContinuousReading::Absent | ContinuousReading::Awaiting { .. } => None,
        }
    }

    pub fn failure(&self) -> Option<&dyn BarWidget> {
        match self {
            ContinuousReading::Failed { failure, .. } => Some(failure),
            ContinuousReading::Absent
            | ContinuousReading::Awaiting { .. }
            | ContinuousReading::Read { .. } => None,
        }
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
        extra: impl IntoIterator<Item = Section>,
    ) -> Option<Tooltip> {
        let own = self.tooltip_with(self.display()?.tooltip())?;
        Some(own.with_sections(extra))
    }
}

impl<S: Source + Occasional> ContinuousReading<S> {
    pub fn present_reading(&self) -> Option<&S> {
        self.get().filter(|data| data.present())
    }

    pub fn present(&self) -> Option<&dyn BarWidget> {
        match self {
            ContinuousReading::Read { data, .. } => {
                data.present().then_some(data as &dyn BarWidget)
            }
            ContinuousReading::Failed { failure, .. } => Some(failure),
            ContinuousReading::Absent | ContinuousReading::Awaiting { .. } => None,
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
