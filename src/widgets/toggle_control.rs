use iced::{Subscription, Task};

use crate::{
    data::source::Source,
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{blocking, polling},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::{Tooltip, TooltipBody},
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
        TooltipBody::sections(&[])
    }
}

pub trait Toggle: Source + BarWidget {
    fn toggled(&self) -> Self;
    fn toggle(&self) -> bool;
}

pub struct ToggleControl<S> {
    cell: LastPoll<S>,
    available: bool,
}

impl<S> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            cell: LastPoll::default(),
            available: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S: Toggle> {
    Available(bool),
    Reading(Result<S, S::Error>),
    Press,
}

impl<S: Toggle> ToggleControl<S> {
    pub fn probe(key: S::Key) -> Task<ToggleMessage<S>> {
        Task::future(async move { ToggleMessage::Available(S::available(&key).await) })
    }

    pub fn subscription(&self, key: S::Key) -> Subscription<ToggleMessage<S>> {
        if !self.available {
            return Subscription::none();
        }
        polling::<S>(key).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Available(available) => {
                self.available = available;
                Task::none()
            }
            ToggleMessage::Reading(reading) => {
                self.cell.receive(reading);
                Task::none()
            }
            ToggleMessage::Press => match self.cell.get().cloned() {
                Some(current) => {
                    self.assume(current.toggled());
                    blocking(S::NAME, move || current.toggle()).discard()
                }
                None => Task::none(),
            },
        }
    }

    pub fn assume(&mut self, value: S) {
        self.cell.load(Some(value));
    }

    pub fn tooltip(&self) -> Option<Tooltip> {
        self.cell.tooltip()
    }

    pub fn reading(&self) -> Option<&dyn BarWidget> {
        self.cell.get().map(|display| display as &dyn BarWidget)
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
