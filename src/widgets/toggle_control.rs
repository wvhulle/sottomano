use iced::{Element, Subscription, Task, widget::button};

use crate::{
    config::Config,
    data::source::Source,
    message_subscriptions::{blocking, polling},
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{
        bar_widget::BarWidget, flat_button, last_poll::LastPoll, source_reading::source_reading,
        tooltip_body::Tooltip,
    },
};

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
    Tooltip(TooltipMessage),
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
            ToggleMessage::Tooltip(_) => Task::none(),
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

    pub fn view<'a>(
        &self,
        config: &Config,
        target: HoverTarget,
        opacity: f32,
    ) -> Option<Element<'a, ToggleMessage<S>>> {
        let display = self.cell.get()?;
        let reading = source_reading(config, Some(display as &dyn BarWidget), opacity)?;
        let hover = hoverable(reading, target, ToggleMessage::Tooltip);
        Some(
            button(hover)
                .padding(0)
                .style(flat_button(config.palette.bar_foreground))
                .on_press(ToggleMessage::Press)
                .into(),
        )
    }
}
