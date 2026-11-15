use std::borrow::Cow;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_run_state::RunChange,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
    systemd_units_journal::{
        ScopedSystemdUnit,
        unit_active_state::{UnitActiveState, UnitActivity, UnitChangeFailure, UnitControl},
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::{CommandOutcome, SubscribedSnapshot},
        value_cells::LabeledValue,
    },
};

#[must_use]
pub(crate) fn activity_text(activity: UnitActivity) -> &'static str {
    match activity {
        UnitActivity::Running => "running",
        UnitActivity::Starting => "starting",
        UnitActivity::Stopping => "stopping",
        UnitActivity::Stopped => "stopped",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceReadiness {
    Serving,
    NotYetServing,
}

#[derive(Clone)]
pub struct UnitSwitchFace {
    pub name: Cow<'static, str>,
    pub icon: Phosphor,
    pub presses: SwitchPresses,
}

impl UnitSwitchFace {
    fn toggle_in(&self, switch_position: SwitchPosition) -> IconToggle {
        IconToggle::new(self.name.clone(), self.icon, switch_position, self.presses)
    }
}

impl From<UnitActivity> for SwitchPosition {
    fn from(activity: UnitActivity) -> Self {
        match activity.press_change() {
            RunChange::Start => SwitchPosition::Off,
            RunChange::Stop => SwitchPosition::On,
        }
    }
}

#[derive(Clone)]
struct UnitSwitchReading {
    toggle: IconToggle,
    shown_glyph: IconGlyph,
    activity: UnitActivity,
}

impl UnitSwitchReading {
    fn shown_state(&self) -> ReadingDisplayState {
        match self.activity {
            UnitActivity::Running => ReadingDisplayState::On,
            UnitActivity::Starting | UnitActivity::Stopping => ReadingDisplayState::Pending,
            UnitActivity::Stopped => ReadingDisplayState::Off,
        }
    }

    fn status_sections(activity: UnitActivity) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new("Service", activity_text(activity))]),
        )]
    }

    fn reading_row(&self) -> ReadingRow<'static> {
        let activity = self.activity;
        ReadingRow::glyph_only(
            self.toggle.name.clone(),
            self.shown_glyph,
            self.shown_state(),
            Box::new(move |_| Self::status_sections(activity)),
        )
    }
}

pub struct UnitSwitch {
    unit: ScopedSystemdUnit,
    face: UnitSwitchFace,
    active_state: SubscribedSnapshot<UnitActiveState>,
}

#[derive(Debug)]
pub enum UnitSwitchMessage {
    ActiveState(SnapshotDelivery<UnitActiveState>),
    Requested {
        change: RunChange,
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
            active_state: SubscribedSnapshot::default(),
        }
    }

    #[must_use]
    pub fn activity(&self) -> Option<UnitActivity> {
        self.active_state.get().map(|state| state.activity)
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<UnitSwitchMessage> {
        snapshot_subscription::<UnitActiveState>(OnBus {
            buses: buses.clone(),
            inputs: self.unit.clone(),
        })
        .map(UnitSwitchMessage::ActiveState)
    }

    pub fn update(&mut self, message: UnitSwitchMessage, clock: Clock) -> Task<UnitSwitchMessage> {
        match message {
            UnitSwitchMessage::ActiveState(delivery) => {
                self.active_state.receive(delivery, clock);
                Task::none()
            }
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

    pub fn press(&mut self) -> Task<UnitSwitchMessage> {
        match self.activity() {
            Some(activity) => self.request(activity.press_change()),
            None => Task::none(),
        }
    }

    pub fn request(&mut self, change: RunChange) -> Task<UnitSwitchMessage> {
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
            toggle: self.face.toggle_in(activity.into()),
            shown_glyph: self.face.icon.with_weight(Weight::Regular).into(),
            activity,
        })
    }

    #[must_use]
    pub fn hover_card(&self, service_readiness: ServiceReadiness) -> Vec<Section> {
        let Some(reading) = self.reading(service_readiness) else {
            return Vec::new();
        };
        self.active_state
            .hover_card_with(UnitSwitchReading::status_sections(reading.activity))
    }

    pub fn group_item<'a, M: 'a>(
        &self,
        service_readiness: ServiceReadiness,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        self.group_item_with_glyph(
            self.face.icon.with_weight(Weight::Regular).into(),
            service_readiness,
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
            Some(reading) => group_item.on_switch_press(
                reading.toggle.switch_position,
                reading.toggle.press_does(),
                pressed,
            ),
            None => group_item,
        }
    }

    pub fn group_item_with_glyph<'a, M: 'a>(
        &self,
        shown_glyph: IconGlyph,
        service_readiness: ServiceReadiness,
        pressed: M,
    ) -> Option<GroupItem<'a, M>> {
        let reading = UnitSwitchReading {
            shown_glyph,
            ..self.reading(service_readiness)?
        };
        let switch_position = reading.toggle.switch_position;
        let press_does = reading.toggle.press_does();
        Some(GroupItem::reading(reading.reading_row()).on_switch_press(
            switch_position,
            press_does,
            pressed,
        ))
    }
}
