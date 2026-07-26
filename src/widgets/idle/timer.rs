//! One idle subsystem's control: its icon, tooltip identity, current timeout
//! (an embedded [`SliderControl`] for the expand/scroll/drag interaction), and
//! a [`ConfirmButton`] that runs the subsystem's action now. Self-contained —
//! the group holds one per subsystem and routes messages to it by kind.

use std::time::Duration;

use iced::{Alignment, Element, widget::Row};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{Tooltip, TooltipBody, row},
    tooltip::HoverTarget,
    widgets::{
        confirm_button::{ConfirmButton, Press},
        icon::Icon,
        idle::{Arm, IdleAction, MAX_POSITION, from_position, label_at, label_for, position},
        slider_control::{Action, ControlMessage, SliderControl},
    },
};

const READOUT_WIDTH: f32 = 30.0;

/// One control's messages: the timeout slider's interaction, or its run-now
/// button.
#[derive(Debug, Clone)]
pub enum TimerMessage {
    Slider(ControlMessage),
    Trigger,
}

/// What a control's update asks the group to do: nothing; close the other
/// controls (this one just opened); persist the committed timeout (the `f32` is
/// the slider position, so the write's confirmation can be routed back); or run
/// this subsystem's action now.
pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
    Fire(IdleAction),
}

pub struct IdleTimer {
    /// The hover target this control emits, e.g. [`HoverTarget::IdleLockUser`].
    target: HoverTarget,
    timeout: Option<Duration>,
    control: SliderControl,
    button: ConfirmButton,
    action: IdleAction,
}

impl IdleTimer {
    pub fn new(icon: Icon, target: HoverTarget, action: IdleAction, confirms: bool) -> Self {
        Self {
            control: SliderControl::new(0.0..=MAX_POSITION, target.clone(), icon)
                .readout_width(READOUT_WIDTH)
                .readout(|value| label_at(value).to_string())
                .label(|value| from_position(value).map_or(String::new(), |d| label_for(d).to_string()))
                .disabled_when(|value| from_position(value).is_none()),
            target,
            timeout: None,
            button: ConfirmButton::new(confirms),
            action,
        }
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// This subsystem's daemon arm, if it has a timeout set (`None` =
    /// disabled).
    pub fn arm(&self) -> Option<Arm> {
        self.timeout.map(|timeout| Arm {
            timeout,
            action: self.action,
        })
    }

    /// Adopt a persisted timeout (on load) and resync the control to it.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
        self.control.update(ControlMessage::Reading(position(timeout)));
    }

    pub fn target(&self) -> &HoverTarget {
        &self.target
    }

    pub fn close(&mut self) {
        self.control.close();
    }

    pub fn disarm(&mut self) {
        self.button.disarm();
    }

    pub fn update(&mut self, msg: TimerMessage) -> TimerChange {
        match msg {
            TimerMessage::Slider(msg) => {
                match self.control.update(msg) {
                    Action::Opened => TimerChange::Opened,
                    Action::Commit(value) => {
                        self.timeout = from_position(value);
                        TimerChange::Persist(value)
                    }
                    Action::None | Action::Right => TimerChange::None,
                }
            }
            // Harmless subsystems fire on the first click; disruptive ones arm,
            // then fire on the confirming second click.
            TimerMessage::Trigger => match self.button.press() {
                Press::Armed => TimerChange::None,
                Press::Fire => TimerChange::Fire(self.action),
            },
        }
    }

    pub fn tooltip(&self) -> Tooltip {
        // The subsystem is already obvious from the pill's icon being hovered,
        // so the tooltip carries only the current timeout state.
        Tooltip {
            body: TooltipBody::Table(vec![row("State", self.timeout.map_or("∞", label_for))]),
            footer: None,
        }
    }

    /// The timeout slider paired with its run-now button, hugging each other (a
    /// tight gap) so the button clearly binds to this subsystem's value.
    pub fn view<'a>(&self, config: &ResolvedConfig, reveal: f32) -> Element<'a, TimerMessage> {
        let slider = self.control.view(config, reveal).map(TimerMessage::Slider);
        let trigger = self.button.view(config, reveal, TimerMessage::Trigger);
        let pair_gap = config.layout.horizontal_widget_spacing_px * 0.25;
        Row::with_children([slider, trigger])
            .spacing(pair_gap)
            .align_y(Alignment::Center)
            .into()
    }
}
