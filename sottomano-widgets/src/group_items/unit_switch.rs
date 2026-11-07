use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{
        ScopedSystemdUnit, SystemdUnitOnBus,
        unit_active_state::{
            UnitActiveState, UnitActivity, UnitChange, UnitChangeFailure, UnitControl,
        },
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{SwitchPosition, SwitchPresses},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{CommandOutcome, HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceReadiness {
    Serving,
    NotYetServing,
}

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

    fn switch_position(self) -> SwitchPosition {
        match self.activity.press_change() {
            UnitChange::Start => SwitchPosition::Off,
            UnitChange::Stop => SwitchPosition::On,
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

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.activity {
            UnitActivity::Running => ReadingDisplayState::On,
            UnitActivity::Starting | UnitActivity::Stopping => ReadingDisplayState::Pending,
            UnitActivity::Stopped => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
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
        change_outcome: Result<(), UnitChangeFailure>,
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

    #[must_use]
    pub fn activity(&self) -> Option<UnitActivity> {
        self.active_state.get().map(|state| state.activity)
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
                change_outcome,
            } => {
                let activity = match change_outcome {
                    Ok(()) => Some(change.settled_activity()),
                    Err(change_failure) => {
                        tracing::warn!(
                            "systemd: {change:?} {}: {change_failure}",
                            self.unit.unit_name
                        );
                        before
                    }
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
            unit_scope: self.unit.scope,
            unit: self.unit.unit_name.clone(),
        };
        Task::perform(control.apply(change), move |change_outcome| {
            UnitSwitchMessage::Requested {
                change,
                before,
                change_outcome,
            }
        })
    }

    fn reading(&self, service_readiness: ServiceReadiness) -> Option<UnitSwitchReading> {
        let activity = match (self.activity()?, service_readiness) {
            (UnitActivity::Running, ServiceReadiness::NotYetServing) => UnitActivity::Starting,
            (activity, _) => activity,
        };
        Some(UnitSwitchReading {
            face: self.face,
            shown_glyph: self.face.icon.with_weight(Weight::Regular).into(),
            activity,
        })
    }

    #[must_use]
    pub fn hover_card(
        &self,
        service_readiness: ServiceReadiness,
        clock: Clock,
    ) -> Option<HoverCard> {
        self.active_state
            .hover_card_with(self.reading(service_readiness)?.hover_card(clock))
    }

    pub fn group_item<'a, M: 'a>(
        &self,
        service_readiness: ServiceReadiness,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        self.group_item_with_glyph(
            self.face.icon.with_weight(Weight::Regular).into(),
            service_readiness,
            target,
            pressed,
        )
    }

    #[must_use]
    pub fn attach_switch_press<'a, M: 'a>(
        &self,
        group_item: GroupItem<'a, M>,
        pressed: M,
    ) -> GroupItem<'a, M> {
        match self.reading(ServiceReadiness::Serving) {
            Some(reading) => {
                group_item.on_switch_press(reading.switch_position(), reading.press_does(), pressed)
            }
            None => group_item,
        }
    }

    pub fn group_item_with_glyph<'a, M: 'a>(
        &self,
        shown_glyph: IconGlyph,
        service_readiness: ServiceReadiness,
        target: impl Into<HoverTarget>,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        let reading = UnitSwitchReading {
            shown_glyph,
            ..self.reading(service_readiness)?
        };
        Some(GroupItem::reading(reading, target).on_switch_press(
            reading.switch_position(),
            reading.press_does(),
            pressed,
        ))
    }
}
