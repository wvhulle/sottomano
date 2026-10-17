use iced::{Subscription, Task};

use crate::{
    data::source::{
        Delivery,
        systemd_manager::{
            UnitScope,
            unit_activity::{UnitActiveState, UnitActivity, UnitChange, UnitControl},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::listening,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::Phosphor,
        toggle_control::SwitchPresses,
        tooltip_body::{KeyValue, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

#[derive(Clone, Copy)]
pub struct UnitSwitchFace {
    pub name: &'static str,
    pub icon: Phosphor,
    pub presses: SwitchPresses,
}

#[derive(Clone, Copy)]
struct UnitSwitchReading {
    face: UnitSwitchFace,
    activity: UnitActivity,
}

impl UnitSwitchReading {
    fn press_does(self) -> &'static str {
        match self.activity.press_change() {
            UnitChange::Start => self.face.presses.turning_on,
            UnitChange::Stop => self.face.presses.turning_off,
        }
    }
}

impl BarWidget for UnitSwitchReading {
    fn name(&self) -> &'static str {
        self.face.name
    }

    fn icon(&self) -> Glyph {
        self.face.icon.with_weight(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        match self.activity {
            UnitActivity::Running => WidgetState::On,
            UnitActivity::Starting | UnitActivity::Stopping => WidgetState::Pending,
            UnitActivity::Stopped => WidgetState::Off,
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new(
            "Service",
            match self.activity {
                UnitActivity::Running => "running",
                UnitActivity::Starting => "starting",
                UnitActivity::Stopping => "stopping",
                UnitActivity::Stopped => "stopped",
            },
        )])
    }
}

pub struct UnitSwitch<S: UnitScope> {
    active_state: ContinuousReading<UnitActiveState<S>>,
}

impl<S: UnitScope> Default for UnitSwitch<S> {
    fn default() -> Self {
        Self {
            active_state: ContinuousReading::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum UnitSwitchMessage<S: UnitScope> {
    ActiveState(Delivery<UnitActiveState<S>>),
    Press,
    Requested {
        before: UnitActivity,
        accepted: bool,
    },
}

impl<S: UnitScope> UnitSwitch<S> {
    fn activity(&self) -> Option<UnitActivity> {
        self.active_state.get().map(|state| state.activity)
    }

    pub fn running(&self) -> bool {
        self.activity().is_some_and(UnitActivity::running)
    }

    pub fn subscription(unit: String) -> Subscription<UnitSwitchMessage<S>> {
        listening::<UnitActiveState<S>>(unit).map(UnitSwitchMessage::ActiveState)
    }

    pub fn update(
        &mut self,
        unit: String,
        message: UnitSwitchMessage<S>,
    ) -> Task<UnitSwitchMessage<S>> {
        match message {
            UnitSwitchMessage::ActiveState(delivery) => {
                self.active_state.push_or_keep(delivery);
                Task::none()
            }
            UnitSwitchMessage::Press => {
                let Some(before) = self.activity() else {
                    return Task::none();
                };
                let change = before.press_change();
                self.active_state
                    .assume(UnitActiveState::from(change.pending_activity()));
                let control = UnitControl {
                    per_user: S::PER_USER,
                    unit,
                };
                Task::perform(control.accepts(change), move |accepted| {
                    UnitSwitchMessage::Requested { before, accepted }
                })
            }
            UnitSwitchMessage::Requested { before, accepted } => {
                if !accepted {
                    self.active_state.assume(UnitActiveState::from(before));
                }
                Task::none()
            }
        }
    }

    fn reading(&self, face: UnitSwitchFace, serving: bool) -> Option<UnitSwitchReading> {
        let activity = match self.activity()? {
            UnitActivity::Running if !serving => UnitActivity::Starting,
            activity => activity,
        };
        Some(UnitSwitchReading { face, activity })
    }

    pub fn tooltip(&self, face: UnitSwitchFace, serving: bool) -> Option<Tooltip> {
        self.active_state
            .tooltip_with(self.reading(face, serving)?.tooltip())
    }

    pub fn group_item<'a, M: 'a>(
        &self,
        face: UnitSwitchFace,
        serving: bool,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        let reading = self.reading(face, serving)?;
        Some(GroupItem::reading(reading, target).on_press(reading.press_does(), pressed))
    }
}
