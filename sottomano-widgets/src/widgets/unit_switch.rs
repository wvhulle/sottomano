use iced::{Subscription, Task};
use sottomano_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        ScopedSystemdUnit, SystemdUnitOnBus,
        unit_active_state::{UnitActiveState, UnitActivity, UnitChange, UnitControl},
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        latest_snapshot::{CommandOutcome, HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::{TooltipBody, tooltip_source_delivery::Tooltip},
        value_cells::LabeledValue,
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
    shown_glyph: IconGlyph,
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

impl GroupItemReading for UnitSwitchReading {
    fn name(&self) -> &'static str {
        self.face.name
    }

    fn icon(&self) -> IconGlyph {
        self.shown_glyph
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self.activity {
            UnitActivity::Running => WidgetState::On,
            UnitActivity::Starting | UnitActivity::Stopping => WidgetState::Pending,
            UnitActivity::Stopped => WidgetState::Off,
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([LabeledValue::new(
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
    active_state: LatestSnapshot<UnitActiveState, HidesFetchFailures>,
}

#[derive(Debug, Clone)]
pub enum UnitSwitchMessage {
    ActiveState(SnapshotDelivery<UnitActiveState>),
    Press,
    Requested {
        change: UnitChange,
        before: Option<UnitActivity>,
        accepted: bool,
    },
}

impl UnitSwitch {
    #[must_use]
    pub fn new(unit: ScopedSystemdUnit, face: UnitSwitchFace) -> Self {
        Self {
            unit,
            face,
            active_state: LatestSnapshot::default(),
        }
    }

    fn activity(&self) -> Option<UnitActivity> {
        self.active_state.get().map(|state| state.activity)
    }

    pub fn running(&self) -> bool {
        self.activity().is_some_and(UnitActivity::running)
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<UnitSwitchMessage> {
        snapshot_subscription::<UnitActiveState>(SystemdUnitOnBus {
            buses: buses.clone(),
            unit: self.unit.clone(),
        })
        .map(UnitSwitchMessage::ActiveState)
    }

    pub fn update(&mut self, message: UnitSwitchMessage, clock: Clock) -> Task<UnitSwitchMessage> {
        match message {
            UnitSwitchMessage::ActiveState(delivery) => {
                self.active_state.receive(delivery, clock);
                Task::none()
            }
            UnitSwitchMessage::Press => match self.activity() {
                Some(activity) => self.request(activity.press_change()),
                None => Task::none(),
            },
            UnitSwitchMessage::Requested {
                change,
                before,
                accepted,
            } => {
                let activity = if accepted {
                    Some(change.settled_activity())
                } else {
                    before
                };
                if let Some(activity) = activity {
                    self.active_state.apply_command(CommandOutcome::Applied {
                        expected_snapshot: UnitActiveState { activity },
                    });
                }
                Task::none()
            }
        }
    }

    pub fn request(&mut self, change: UnitChange) -> Task<UnitSwitchMessage> {
        let before = self.activity();
        self.active_state.apply_command(CommandOutcome::Applied {
            expected_snapshot: UnitActiveState {
                activity: change.pending_activity(),
            },
        });
        let control = UnitControl {
            per_user: self.unit.scope.per_user(),
            unit: self.unit.unit_name.clone(),
        };
        Task::perform(control.accepts(change), move |accepted| {
            UnitSwitchMessage::Requested {
                change,
                before,
                accepted,
            }
        })
    }

    fn reading(&self, serving: bool) -> Option<UnitSwitchReading> {
        let activity = match self.activity()? {
            UnitActivity::Running if !serving => UnitActivity::Starting,
            activity => activity,
        };
        Some(UnitSwitchReading {
            face: self.face,
            shown_glyph: self.face.icon.with_weight(Weight::Regular).into(),
            activity,
        })
    }

    #[must_use]
    pub fn tooltip(&self, serving: bool, clock: Clock) -> Option<Tooltip> {
        self.active_state
            .tooltip_with(self.reading(serving)?.tooltip(clock))
    }

    pub fn group_item<'a, M: 'a>(
        &self,
        serving: bool,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        self.group_item_with_glyph(
            self.face.icon.with_weight(Weight::Regular).into(),
            serving,
            target,
            pressed,
        )
    }

    pub fn group_item_with_glyph<'a, M: 'a>(
        &self,
        shown_glyph: IconGlyph,
        serving: bool,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        let reading = UnitSwitchReading {
            shown_glyph,
            ..self.reading(serving)?
        };
        Some(GroupItem::reading(reading, target).on_press(reading.press_does(), pressed))
    }
}
