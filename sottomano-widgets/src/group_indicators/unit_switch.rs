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
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_interaction::IndicatorGroupInteraction,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
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
pub struct UnitSwitchIndicator {
    toggle: IconToggle,
    shown_glyph: IconGlyph,
    activity: UnitActivity,
}

impl UnitSwitchIndicator {
    #[must_use]
    pub fn with_glyph(self, shown_glyph: IconGlyph) -> Self {
        Self {
            shown_glyph,
            ..self
        }
    }

    pub fn wired_switch_indicator<'view>(
        self,
        pressed: impl Into<IndicatorGroupInteraction>,
    ) -> GroupIndicator<'view> {
        let switch_position = self.toggle.switch_position;
        let press_does = self.toggle.press_does();
        self.indicator()
            .on_switch_press(switch_position, press_does, pressed)
    }

    fn shown_state(&self) -> IndicatorState {
        match self.activity {
            UnitActivity::Running => IndicatorState::On,
            UnitActivity::Starting | UnitActivity::Stopping => IndicatorState::Pending,
            UnitActivity::Stopped => IndicatorState::Off,
        }
    }

    fn status_sections(activity: UnitActivity) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new("Service", activity_text(activity))]),
        )]
    }

    fn indicator<'indicator>(&self) -> GroupIndicator<'indicator> {
        let activity = self.activity;
        GroupIndicator::glyph_only(
            self.toggle.name.clone(),
            self.shown_glyph,
            self.shown_state(),
        )
        .with_hover_card(Box::new(move |_| Self::status_sections(activity)))
    }
}

pub struct UnitSwitch {
    unit: ScopedSystemdUnit,
    face: UnitSwitchFace,
    active_state: SubscribedSnapshot<UnitActiveState>,
}

#[derive(Debug, Clone, Copy)]
pub enum RecordedActivity {
    Unrecorded,
    Recorded(UnitActivity),
}

#[derive(Debug)]
pub enum UnitSwitchMessage {
    ActiveState(SnapshotDelivery<UnitActiveState>),
    Requested {
        change: RunChange,
        before: RecordedActivity,
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

    fn recorded_activity(&self) -> RecordedActivity {
        match self.active_state.get() {
            Some(state) => RecordedActivity::Recorded(state.activity),
            None => RecordedActivity::Unrecorded,
        }
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
                    Ok(()) => RecordedActivity::Recorded(change.settled_activity()),
                    Err(change_failure) => {
                        tracing::warn!(
                            "systemd: {change:?} {}: {change_failure}",
                            self.unit.unit_name
                        );
                        before
                    }
                };
                if let RecordedActivity::Recorded(activity) = activity {
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
        let before = self.recorded_activity();
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

    #[must_use]
    pub fn switch_indicator(
        &self,
        service_readiness: ServiceReadiness,
    ) -> Option<UnitSwitchIndicator> {
        let activity = match (self.activity()?, service_readiness) {
            (UnitActivity::Running, ServiceReadiness::NotYetServing) => UnitActivity::Starting,
            (activity, _) => activity,
        };
        Some(UnitSwitchIndicator {
            toggle: self.face.toggle_in(activity.into()),
            shown_glyph: self.face.icon.with_weight(Weight::Regular).into(),
            activity,
        })
    }

    #[must_use]
    pub fn hover_card(&self, service_readiness: ServiceReadiness) -> Vec<Section> {
        let Some(sample) = self.switch_indicator(service_readiness) else {
            return Vec::new();
        };
        self.active_state
            .hover_card_with(UnitSwitchIndicator::status_sections(sample.activity))
    }

    #[must_use]
    pub fn attach_switch_press<'view>(
        &self,
        group_indicator: GroupIndicator<'view>,
        pressed: impl Into<IndicatorGroupInteraction>,
    ) -> GroupIndicator<'view> {
        match self.switch_indicator(ServiceReadiness::Serving) {
            Some(sample) => group_indicator.on_switch_press(
                sample.toggle.switch_position,
                sample.toggle.press_does(),
                pressed,
            ),
            None => group_indicator,
        }
    }
}
