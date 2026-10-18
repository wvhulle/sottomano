use iced::{Subscription, Task};
use vetrata_sources::{ReadingSource, SourceDelivery, background_work::blocking};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::Phosphor,
        tooltip_body::{Freshness, Section, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

#[derive(Clone, Copy)]
pub struct SwitchPresses {
    pub turning_on: &'static str,
    pub turning_off: &'static str,
}

#[derive(Clone, Copy)]
pub struct IconToggle {
    pub name: &'static str,
    pub icon: Phosphor,
    pub enabled: bool,
    pub presses: SwitchPresses,
}

impl IconToggle {
    #[must_use]
    pub fn new(name: &'static str, icon: Phosphor, enabled: bool, presses: SwitchPresses) -> Self {
        Self {
            name,
            icon,
            enabled,
            presses,
        }
    }

    #[must_use]
    pub fn press_does(&self) -> &'static str {
        if self.enabled {
            self.presses.turning_off
        } else {
            self.presses.turning_on
        }
    }

    #[must_use]
    pub fn switch_tooltip(&self) -> Tooltip {
        Tooltip::sourced(self.tooltip(), Freshness::Live)
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

pub trait ToggleableSource: ReadingSource<Error: Into<Section>> + BarWidget {
    #[must_use]
    fn toggled_value(&self) -> Self;
    fn apply_toggle(&self) -> bool;
    fn press_action_text(&self) -> &'static str;
}

pub struct ToggleControl<S: ReadingSource> {
    reading: ContinuousReading<S>,
}

impl<S: ReadingSource> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            reading: ContinuousReading::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S: ToggleableSource> {
    Reading(SourceDelivery<S>),
    Press,
}

impl<S: ToggleableSource> ToggleControl<S> {
    pub fn subscription(inputs: S::StreamInputs) -> Subscription<ToggleMessage<S>> {
        source_subscription::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn refresh(inputs: S::StreamInputs) -> Task<ToggleMessage<S>> {
        first_delivery::<S>(inputs).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Reading(reading) => {
                self.reading.receive_delivery(reading);
                Task::none()
            }
            ToggleMessage::Press => match self.reading.get().cloned() {
                Some(current) => {
                    self.assume(current.toggled_value());
                    Task::future(blocking(S::NAME, move || current.apply_toggle())).discard()
                }
                None => Task::none(),
            },
        }
    }

    pub fn assume(&mut self, value: S) {
        self.reading.assume(value);
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        match self.reading.get() {
            Some(_) => self.reading.tooltip_with(TooltipBody::sections([])),
            None => self.reading.tooltip(),
        }
    }

    pub fn reading(&self) -> Option<&dyn BarWidget> {
        self.reading.get().map(|display| display as &dyn BarWidget)
    }

    pub fn group_item<'a, M: 'a>(
        &'a self,
        target: impl Into<HoverTarget>,
        on_toggle: fn(ToggleMessage<S>) -> M,
    ) -> Option<GroupItem<'a, M>> {
        self.reading.get().map(|current| {
            GroupItem::reading(current, target)
                .on_press(current.press_action_text(), on_toggle(ToggleMessage::Press))
        })
    }
}
