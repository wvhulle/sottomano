use iced::{Subscription, Task};

use crate::{
    data::source::{Delivery, PolledSource},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, blocking, polling},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Provenance, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

pub const TOGGLE_PRESS_HINT: &str = "switch on or off";

#[derive(Clone, Copy)]
pub struct IconToggle {
    pub name: &'static str,
    pub icon: Phosphor,
    pub enabled: bool,
}

impl IconToggle {
    pub fn new(name: &'static str, icon: Phosphor, enabled: bool) -> Self {
        Self {
            name,
            icon,
            enabled,
        }
    }

    pub fn readout_tooltip(label: impl Into<Cell>, readout: impl Into<Cell>) -> Tooltip {
        Tooltip::sourced(
            TooltipBody::dictionary([KeyValue::new(label, readout)]),
            Provenance::Live,
        )
    }

    pub fn switch_tooltip(&self, label: impl Into<Cell>, described: &str) -> Tooltip {
        Self::readout_tooltip(label, if self.enabled { described } else { "off" })
    }
}

impl BarWidget for IconToggle {
    fn name(&self) -> &'static str {
        self.name
    }

    fn icon(&self) -> Glyph {
        self.icon.with_weight(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.enabled)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([])
    }
}

pub trait Toggle: PolledSource + BarWidget {
    fn toggled(&self) -> Self;
    fn toggle(&self) -> bool;
}

pub struct ToggleControl<S> {
    reading: ContinuousReading<S>,
}

impl<S> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            reading: ContinuousReading::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S: Toggle> {
    Reading(Delivery<S>),
    Press,
}

impl<S: Toggle> ToggleControl<S> {
    pub fn subscription(
        key: S::Key,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<ToggleMessage<S>> {
        polling::<S>(key, forced).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Reading(reading) => {
                self.reading.receive(reading);
                Task::none()
            }
            ToggleMessage::Press => match self.reading.get().cloned() {
                Some(current) => {
                    self.assume(current.toggled());
                    blocking(S::NAME, move || current.toggle()).discard()
                }
                None => Task::none(),
            },
        }
    }

    pub fn assume(&mut self, value: S) {
        self.reading.assume(value);
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.reading.tooltip()
    }

    pub fn reading(&self) -> Option<&dyn BarWidget> {
        self.reading.get().map(|display| display as &dyn BarWidget)
    }

    pub fn group_item<'a, M: 'a>(
        &'a self,
        target: impl Into<HoverTarget>,
        on_toggle: fn(ToggleMessage<S>) -> M,
    ) -> Option<GroupItem<'a, M>> {
        self.reading().map(|reading| {
            GroupItem::reading(reading, target)
                .on_press(TOGGLE_PRESS_HINT, on_toggle(ToggleMessage::Press))
        })
    }
}
