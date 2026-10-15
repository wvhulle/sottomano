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
    stamped: Option<StampedReading<S>>,
}

struct StampedReading<S> {
    freshness: Freshness,
    outcome: ReadingOutcome<S>,
}

enum ReadingOutcome<S> {
    Awaiting,
    Read(S),
    Failed(FailedReading),
}

impl<S> ReadingOutcome<S> {
    fn data(&self) -> Option<&S> {
        match self {
            ReadingOutcome::Read(data) => Some(data),
            ReadingOutcome::Awaiting | ReadingOutcome::Failed(_) => None,
        }
    }

    fn keep<E: Display>(self, reading: Result<S, E>, source: &'static str) -> Self {
        match reading {
            Ok(data) => ReadingOutcome::Read(data),
            Err(failure) => {
                tracing::warn!("{source}: {failure}");
                self
            }
        }
    }
}

impl<S: BarWidget> ReadingOutcome<S> {
    fn accept<E: Display>(self, reading: Result<S, E>) -> Self {
        match reading {
            Ok(data) => ReadingOutcome::Read(data),
            Err(failure) => ReadingOutcome::Failed(FailedReading {
                icon: match self {
                    ReadingOutcome::Read(data) => data.icon(),
                    ReadingOutcome::Failed(failed) => failed.icon,
                    ReadingOutcome::Awaiting => S::fallback_icon(),
                },
                message: failure.to_string(),
            }),
        }
    }
}

impl<S> Default for ContinuousReading<S> {
    fn default() -> Self {
        Self { stamped: None }
    }
}

impl<S> ContinuousReading<S> {
    pub fn get(&self) -> Option<&S> {
        self.stamped.as_ref()?.outcome.data()
    }

    pub fn assume(&mut self, data: S) {
        if let Some(stamped) = &mut self.stamped {
            stamped.outcome = ReadingOutcome::Read(data);
        }
    }

    pub fn tooltip_with(&self, body: TooltipBody) -> Option<Tooltip> {
        let tooltip = Tooltip::sourced(body, self.stamped.as_ref()?.freshness);
        (!tooltip.is_empty()).then_some(tooltip)
    }

    pub fn sections(&self, sections: impl FnOnce(&S) -> Vec<Section>) -> Vec<SourcedSection> {
        let Some(stamped) = &self.stamped else {
            return Vec::new();
        };
        stamped
            .outcome
            .data()
            .map(sections)
            .unwrap_or_default()
            .into_iter()
            .map(|section| SourcedSection::new(section, Readiness::Settled, stamped.freshness))
            .collect()
    }
}

impl<S: Source> ContinuousReading<S> {
    fn settle(
        &mut self,
        delivery: Delivery<S>,
        freshness: Freshness,
        settle: impl FnOnce(ReadingOutcome<S>, Result<S, S::Error>) -> ReadingOutcome<S>,
    ) {
        self.stamped = match delivery {
            Delivery::Absent => None,
            Delivery::Reading(reading) => {
                let previous = self
                    .stamped
                    .take()
                    .map_or(ReadingOutcome::Awaiting, |stamped| stamped.outcome);
                Some(StampedReading {
                    freshness,
                    outcome: settle(previous, reading),
                })
            }
        };
    }
}

impl<S: LiveSource> ContinuousReading<S> {
    pub fn load(&mut self, data: S) {
        self.stamped = Some(StampedReading {
            freshness: Freshness::Live,
            outcome: ReadingOutcome::Read(data),
        });
    }

    pub fn push_or_keep(&mut self, delivery: Delivery<S>) {
        self.settle(delivery, Freshness::Live, |previous, reading| {
            previous.keep(reading, S::NAME)
        });
    }
}

impl<S: LiveSource + BarWidget> ContinuousReading<S> {
    pub fn push(&mut self, delivery: Delivery<S>) {
        self.settle(delivery, Freshness::Live, ReadingOutcome::accept);
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
        self.settle(delivery, Self::polled_now(), |previous, reading| {
            previous.keep(reading, S::NAME)
        });
    }
}

impl<S: PolledSource + BarWidget> ContinuousReading<S> {
    pub fn receive(&mut self, delivery: Delivery<S>) {
        self.settle(delivery, Self::polled_now(), ReadingOutcome::accept);
    }
}

impl<S: BarWidget> ContinuousReading<S> {
    pub fn display(&self) -> Option<&dyn BarWidget> {
        match &self.stamped.as_ref()?.outcome {
            ReadingOutcome::Read(data) => Some(data),
            ReadingOutcome::Failed(failure) => Some(failure),
            ReadingOutcome::Awaiting => None,
        }
    }

    pub fn failure(&self) -> Option<&dyn BarWidget> {
        match &self.stamped.as_ref()?.outcome {
            ReadingOutcome::Failed(failure) => Some(failure),
            ReadingOutcome::Awaiting | ReadingOutcome::Read(_) => None,
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
        extra: impl IntoIterator<Item = SourcedSection>,
    ) -> Option<Tooltip> {
        let own = self.tooltip_with(self.display()?.tooltip())?;
        Some(own.with_sections(extra))
    }
}

impl<S: Occasional> ContinuousReading<S> {
    pub fn present_reading(&self) -> Option<&S> {
        self.get().filter(|data| data.present())
    }

    pub fn present(&self) -> Option<&dyn BarWidget> {
        match &self.stamped.as_ref()?.outcome {
            ReadingOutcome::Read(data) => data.present().then_some(data as &dyn BarWidget),
            ReadingOutcome::Failed(failure) => Some(failure),
            ReadingOutcome::Awaiting => None,
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
