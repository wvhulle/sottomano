use iced::{Subscription, Task};

use crate::{
    data::source::{
        SourceDelivery,
        systemd_manager::{
            ScopedSystemdUnit,
            unit_activity::{UnitActiveState, UnitActivity, UnitChange, UnitControl},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::source_subscription,
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

pub struct UnitSwitch {
    unit: ScopedSystemdUnit,
    face: UnitSwitchFace,
    active_state: ContinuousReading<UnitActiveState>,
}

#[derive(Debug, Clone)]
pub enum UnitSwitchMessage {
    ActiveState(SourceDelivery<UnitActiveState>),
    Press,
    Requested {
        before: Option<UnitActivity>,
        accepted: bool,
    },
}

impl UnitSwitch {
    pub fn new(unit: ScopedSystemdUnit, face: UnitSwitchFace) -> Self {
        Self {
            unit,
            face,
            active_state: ContinuousReading::default(),
        }
    }

    fn activity(&self) -> Option<UnitActivity> {
        self.active_state.get().map(|state| state.activity)
    }

    pub fn running(&self) -> bool {
        self.activity().is_some_and(UnitActivity::running)
    }

    pub fn subscription(&self) -> Subscription<UnitSwitchMessage> {
        source_subscription::<UnitActiveState>(self.unit.clone(), None)
            .map(UnitSwitchMessage::ActiveState)
    }

    pub fn update(&mut self, message: UnitSwitchMessage) -> Task<UnitSwitchMessage> {
        match message {
            UnitSwitchMessage::ActiveState(delivery) => {
                self.active_state.receive_keeping_last_reading(delivery);
                Task::none()
            }
            UnitSwitchMessage::Press => match self.activity() {
                Some(activity) => self.request(activity.press_change()),
                None => Task::none(),
            },
            UnitSwitchMessage::Requested { before, accepted } => {
                if let Some(activity) = before.filter(|_| !accepted) {
                    self.active_state.assume(UnitActiveState { activity });
                }
                Task::none()
            }
        }
    }

    pub fn request(&mut self, change: UnitChange) -> Task<UnitSwitchMessage> {
        let before = self.activity();
        self.active_state.assume(UnitActiveState {
            activity: change.pending_activity(),
        });
        let control = UnitControl {
            per_user: self.unit.scope.per_user(),
            unit: self.unit.unit_name.clone(),
        };
        Task::perform(control.accepts(change), move |accepted| {
            UnitSwitchMessage::Requested { before, accepted }
        })
    }

    fn reading(&self, serving: bool) -> Option<UnitSwitchReading> {
        let activity = match self.activity()? {
            UnitActivity::Running if !serving => UnitActivity::Starting,
            activity => activity,
        };
        Some(UnitSwitchReading {
            face: self.face,
            activity,
        })
    }

    pub fn tooltip(&self, serving: bool) -> Option<Tooltip> {
        self.active_state
            .tooltip_with(self.reading(serving)?.tooltip())
    }

    pub fn group_item<'a, M: 'a>(
        &self,
        serving: bool,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        let reading = self.reading(serving)?;
        Some(GroupItem::reading(reading, target).on_press(reading.press_does(), pressed))
    }
}
