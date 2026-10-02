use iced::{Element, Subscription, Task};

use crate::{
    config::Config,
    data::source::Source,
    desktop_shell::hover_tooltips::{HoverTarget, hoverable},
    message_subscriptions::{blocking, polling},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::Phosphor,
        tooltip_body::Tooltip,
        widget_group::GroupEvent,
        widget_state::WidgetState,
    },
};

pub fn icon_toggle_glyph<'a, M: 'a>(
    config: &Config,
    icon: Phosphor,
    enabled: bool,
) -> Element<'a, M> {
    let state = WidgetState::of_toggle(enabled);
    Glyph::from(icon.with_weight(state.weight()))
        .view(config.font.icon_size_px, state.color(&config.palette))
}

pub fn icon_toggle_item<M: Clone + 'static>(
    config: &Config,
    icon: Phosphor,
    enabled: bool,
    press: M,
    target: HoverTarget,
) -> GroupItem<'_, M> {
    GroupItem::hoverable_element(move |_| {
        hoverable(
            icon_toggle_glyph(config, icon, enabled),
            target.clone(),
            GroupEvent::Tooltip,
        )
    })
    .on_press(press)
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
}
