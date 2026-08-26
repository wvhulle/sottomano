use iced::{Element, Subscription, Task, widget::button};

use crate::{
    config::Config,
    data::{
        source::{ApiSource, BarWidget, Tooltip},
        source_cell::SourceCell,
    },
    message_subscriptions::{blocking, poll_arg},
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widgets::{flat_button, source_reading::source_reading},
};

pub trait Toggle: ApiSource {
    fn toggled(&self) -> Self;
    fn toggle(&self) -> bool;
}

pub struct ToggleControl<S> {
    cell: SourceCell<S>,
}

impl<S> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            cell: SourceCell::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S: Toggle> {
    Reading(Result<Option<S>, S::Error>),
    Press,
    Tooltip(TooltipMessage),
}

impl<S: Toggle> ToggleControl<S> {
    pub fn subscription(key: S::Key) -> Subscription<ToggleMessage<S>> {
        poll_arg::<S>(key).map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>) -> Task<ToggleMessage<S>> {
        match message {
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
        let tooltip = Tooltip {
            body: self.cell.display()?.tooltip(),
            footer: self.cell.footer(S::INTERVAL),
        };
        (!tooltip.is_empty()).then_some(tooltip)
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
