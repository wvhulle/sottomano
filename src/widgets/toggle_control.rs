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
    pub fn new(name: &'static str, icon: Phosphor, enabled: bool, presses: SwitchPresses) -> Self {
        Self {
            name,
            icon,
            enabled,
            presses,
        }
    }

    pub fn press_does(&self) -> &'static str {
        if self.enabled {
            self.presses.turning_off
        } else {
            self.presses.turning_on
        }
    }

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

pub trait Toggle: PolledSource + BarWidget {
    fn toggled(&self) -> Self;
    fn toggle(&self) -> bool;
    fn press_does(&self) -> &'static str;
    fn facts() -> Vec<Section>;
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
        self.tooltip_with_facts(S::facts())
    }

    pub fn tooltip_with_facts(&self, facts: Vec<Section>) -> Option<Tooltip> {
        match self.reading.get() {
            Some(_) => self.reading.tooltip_with(TooltipBody::sections(facts)),
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
                .on_press(current.press_does(), on_toggle(ToggleMessage::Press))
        })
    }
}
