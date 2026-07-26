//! A stateful icon toggle (odin's `CommandIndicator`): an icon reflecting a
//! polled boolean source that flips it on click. The widget owns its reading,
//! its refresh subscription, the optimistic flip shown until the next poll, the
//! click command, and its own view — so a group composing it carries no
//! toggle-specific logic. Reused for clight auto-brightness and umbra
//! dark-mode.

use iced::{Element, Subscription, Task, widget::mouse_area};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Source},
    message_subscriptions::blocking,
    tooltip::{TooltipMessage, TooltipSource, hoverable},
    widgets::icon_gauge::source_reading,
};

/// A [`Source`] whose click action and next state the widget can drive itself:
/// [`toggled`](Toggle::toggled) predicts the optimistic frame, [`toggle`] runs
/// the side effect (D-Bus / CLI) off-thread.
///
/// [`toggle`]: Toggle::toggle
pub trait Toggle: Source + Clone {
    /// The predicted state right after a click, shown until the next poll
    /// lands.
    fn toggled(&self) -> Self;
    /// Run the toggle side effect; `true` on success.
    fn toggle() -> bool;
}

pub struct ToggleControl<S> {
    cell: SourceCell<S>,
    /// Optimistic override shown from click until the next reading reconciles
    /// it.
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
                // Flip optimistically for instant feedback; the next poll reconciles.
                self.pending = self.cell.get().map(S::toggled);
                blocking(S::NAME, S::toggle).discard()
            }
            ToggleMessage::Tooltip(_) => Task::none(),
        }
    }

    pub fn cell(&self) -> &SourceCell<S> {
        &self.cell
    }

    /// The toggle's icon (optimistic state preferred), hover-tooltipped and
    /// clickable. `None` until the first reading — an absent source hides it.
    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        source: TooltipSource,
        alpha: f32,
    ) -> Option<Element<'a, ToggleMessage<S>>> {
        let display = self.pending.as_ref().or_else(|| self.cell.get())?;
        let reading = source_reading(config, Some(display as &dyn BarWidget), alpha)?;
        let hover = hoverable(reading, source, ToggleMessage::Tooltip);
        Some(mouse_area(hover).on_press(ToggleMessage::Press).into())
    }
}
