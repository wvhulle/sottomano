//! One idle-timeout pill: a subsystem's icon, tooltip identity, current timeout,
//! and an embedded [`SliderControl`] for the expand/scroll/drag interaction. The
//! group holds four and addresses them by kind.

use std::{ops::RangeInclusive, time::Duration};

use iced::Element;

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, Tooltip, TooltipBody, row},
    tooltip::TooltipSource,
    widgets::{
        icon::Icon,
        idle::{MAX_POSITION, from_position, label_at, label_for, position},
        slider_control::{Action, ControlMessage, SliderControl, View},
    },
};

const SCROLL_STEP: f32 = 1.0;
const READOUT_WIDTH: f32 = 30.0;

/// What a timer's update asks the group to do: nothing, close the other pills
/// (this one just opened), or persist the timeouts. `Persist` carries the
/// committed slider position so the write's confirmation can be routed back.
pub enum TimerChange {
    None,
    Opened,
    Persist(f32),
}

pub struct IdleTimer {
    icon: Icon,
    source: TooltipSource,
    timeout: Option<Duration>,
    control: SliderControl,
}

impl IdleTimer {
    pub fn new(icon: Icon, source: TooltipSource) -> Self {
        Self {
            icon,
            source,
            timeout: None,
            control: SliderControl::default(),
        }
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Adopt a persisted timeout (on load) and resync the control to it.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
        self.control.sync(position(timeout));
    }

    pub fn source(&self) -> TooltipSource {
        self.source
    }

    pub fn close(&mut self) {
        self.control.close();
    }

    fn range() -> RangeInclusive<f32> {
        0.0..=MAX_POSITION
    }

    pub fn update(&mut self, msg: ControlMessage) -> TimerChange {
        let reading = position(self.timeout);
        match self.control.update(msg, reading, &Self::range(), SCROLL_STEP) {
            Action::Opened => TimerChange::Opened,
            Action::Commit(value) => {
                self.timeout = from_position(value);
                TimerChange::Persist(value)
            }
            Action::None | Action::Right => TimerChange::None,
        }
    }

    pub fn tooltip(&self) -> Tooltip {
        let name = match self.source {
            TooltipSource::IdleLockUser => "lock-user",
            TooltipSource::IdleScreenOff => "screen-off",
            TooltipSource::IdleStandby => "standby",
            TooltipSource::IdlePowerDown => "power-down",
            _ => "idle",
        };
        Tooltip {
            body: TooltipBody::Table(vec![
                row("Subsystem", name),
                row("State", self.timeout.map_or("∞", label_for)),
            ]),
            footer: None,
        }
    }

    pub fn view<'a>(&self, config: &ResolvedConfig, reveal: f32) -> Element<'a, ControlMessage> {
        let value = self.control.value(position(self.timeout));
        let widget = Reading {
            icon: self.icon,
            label: from_position(value).map(|d| label_for(d).to_string()),
            enabled: from_position(value).is_some(),
        };
        self.control.view(
            config,
            View {
                widget: &widget,
                value,
                readout: label_at(value).to_string(),
                readout_width: READOUT_WIDTH,
                range: Self::range(),
                step: SCROLL_STEP,
                source: self.source,
                right_click: false,
                alpha: reveal,
            },
        )
    }
}

/// A closed pill's icon + label, rendered through the shared reading path.
struct Reading {
    icon: Icon,
    label: Option<String>,
    enabled: bool,
}

impl BarWidget for Reading {
    fn icon(&self) -> Icon {
        self.icon
    }

    fn disabled(&self) -> bool {
        !self.enabled
    }

    fn label(&self) -> Option<String> {
        self.label.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}
