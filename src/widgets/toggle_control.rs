use iced::{Element, Subscription, Task, widget::mouse_area};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{BarWidget, Source},
    message_subscriptions::blocking,
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widget_groups::source_cell::SourceCell,
    widgets::icon_gauge::source_reading,
};

pub trait Toggle: Source + Clone {
    fn toggled(&self) -> Self;
    fn toggle() -> bool;
}

pub struct ToggleControl<S> {
    cell: SourceCell<S>,
    pending: Option<S>,
}

impl<S> Default for ToggleControl<S> {
    fn default() -> Self {
        Self {
            cell: SourceCell::default(),
            pending: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToggleMessage<S> {
    Reading(Option<S>),
    Press,
    Tooltip(TooltipMessage),
}

impl<S: Toggle + Send + 'static> ToggleControl<S> {
    pub fn subscription() -> Subscription<ToggleMessage<S>> {
        SourceCell::<S>::subscription().map(ToggleMessage::Reading)
    }

    pub fn update(&mut self, message: ToggleMessage<S>) -> Task<ToggleMessage<S>> {
        match message {
            ToggleMessage::Reading(reading) => {
                self.cell.load(reading);
                self.pending = None;
                Task::none()
            }
            ToggleMessage::Press => {
                self.pending = self.cell.get().map(S::toggled);
                blocking(S::NAME, S::toggle).discard()
            }
            ToggleMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn cell(&self) -> &SourceCell<S> {
        &self.cell
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        target: HoverTarget,
        alpha: f32,
    ) -> Option<Element<'a, ToggleMessage<S>>> {
        let display = self.pending.as_ref().or_else(|| self.cell.get())?;
        let reading = source_reading(config, Some(display as &dyn BarWidget), alpha)?;
        let hover = hoverable(reading, target, ToggleMessage::Tooltip);
        Some(mouse_area(hover).on_press(ToggleMessage::Press).into())
    }
}
