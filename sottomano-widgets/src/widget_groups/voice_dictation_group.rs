mod voxtype_dictation_reading;

use iced::{Subscription, Task};
use sottomano_sources::{
    SourceDelivery,
    blocking_background_work::blocking,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::voxtype_daemon_state::{
        VoxtypeDictation, cancel_recording, toggle_recording,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::voice_dictation_group::voxtype_dictation_reading::{
        CancelDictation, RECOGNIZER_DAEMON, TYPING_DAEMON,
    },
    widgets::{
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        unit_switch::{UnitSwitch, UnitSwitchMessage},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

const RECOGNIZER_UNIT: &str = "voxtype.service";
const TYPING_UNIT: &str = "dotoold.service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    Dictation,
    Cancel,
    RecognizerDaemon,
    TypingDaemon,
}

impl From<VoiceDictationTarget> for HoverTarget {
    fn from(target: VoiceDictationTarget) -> Self {
        HoverTarget::VoiceDictation(target)
    }
}

pub struct VoiceDictationGroup {
    dictation: ContinuousReading<VoxtypeDictation>,
    recognizer_daemon: UnitSwitch,
    typing_daemon: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VoiceDictationMessage {
    Dictation(SourceDelivery<VoxtypeDictation>),
    ToggleRecording,
    CancelRecording,
    RecognizerDaemon(UnitSwitchMessage),
    TypingDaemon(UnitSwitchMessage),
    Expansion(ExpansionMessage),
}

impl Default for VoiceDictationGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl VoiceDictationGroup {
    #[must_use]
    pub fn new() -> Self {
        let user_unit = |unit_name: &str| ScopedSystemdUnit {
            scope: SystemdUnitScope::UserSession,
            unit_name: unit_name.to_owned(),
        };
        Self {
            dictation: ContinuousReading::default(),
            recognizer_daemon: UnitSwitch::new(user_unit(RECOGNIZER_UNIT), RECOGNIZER_DAEMON),
            typing_daemon: UnitSwitch::new(user_unit(TYPING_UNIT), TYPING_DAEMON),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            source_subscription::<VoxtypeDictation>(()).map(VoiceDictationMessage::Dictation),
            self.recognizer_daemon
                .subscription(buses)
                .map(VoiceDictationMessage::RecognizerDaemon),
            self.typing_daemon
                .subscription(buses)
                .map(VoiceDictationMessage::TypingDaemon),
        ])
    }

    pub fn refresh(target: VoiceDictationTarget) -> Task<VoiceDictationMessage> {
        match target {
            VoiceDictationTarget::Dictation
            | VoiceDictationTarget::Cancel
            | VoiceDictationTarget::RecognizerDaemon
            | VoiceDictationTarget::TypingDaemon => {
                first_delivery::<VoxtypeDictation>(()).map(VoiceDictationMessage::Dictation)
            }
        }
    }

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(
        &mut self,
        msg: VoiceDictationMessage,
        clock: Clock,
    ) -> Task<VoiceDictationMessage> {
        match msg {
            VoiceDictationMessage::Dictation(delivery) => {
                self.dictation.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ToggleRecording => {
                Task::future(blocking("voxtype record toggle", toggle_recording)).discard()
            }
            VoiceDictationMessage::CancelRecording => {
                Task::future(blocking("voxtype record cancel", cancel_recording)).discard()
            }
            VoiceDictationMessage::RecognizerDaemon(m) => self
                .recognizer_daemon
                .update(m, clock)
                .map(VoiceDictationMessage::RecognizerDaemon),
            VoiceDictationMessage::TypingDaemon(m) => self
                .typing_daemon
                .update(m, clock)
                .map(VoiceDictationMessage::TypingDaemon),
            VoiceDictationMessage::Expansion(m) => {
                self.expansion.update(m);
                Task::none()
            }
        }
    }

    fn dictation_busy(&self) -> bool {
        self.dictation
            .get()
            .is_some_and(|dictation| dictation.activity.busy())
    }

    #[must_use]
    pub fn tooltip_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            VoiceDictationTarget::Dictation | VoiceDictationTarget::Cancel => {
                self.dictation.tooltip(clock)
            }
            VoiceDictationTarget::RecognizerDaemon => self.recognizer_daemon.tooltip(true, clock),
            VoiceDictationTarget::TypingDaemon => self.typing_daemon.tooltip(true, clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, VoiceDictationMessage> {
        let dictation = self.dictation.get();
        GroupContent::new(
            &self.expansion,
            VoiceDictationMessage::Expansion,
            [
                self.dictation
                    .group_item(VoiceDictationTarget::Dictation)
                    .map(|item| {
                        item.on_press(
                            dictation.map_or(
                                "start dictating at the cursor",
                                voxtype_dictation_reading::press_does,
                            ),
                            VoiceDictationMessage::ToggleRecording,
                        )
                        .previewed_on_handle()
                    }),
                self.dictation_busy().then(|| {
                    GroupItem::reading(CancelDictation, VoiceDictationTarget::Cancel).on_press(
                        "discard this dictation without typing anything more",
                        VoiceDictationMessage::CancelRecording,
                    )
                }),
                self.recognizer_daemon.group_item(
                    true,
                    VoiceDictationTarget::RecognizerDaemon,
                    VoiceDictationMessage::RecognizerDaemon(UnitSwitchMessage::Press),
                ),
                self.typing_daemon.group_item(
                    true,
                    VoiceDictationTarget::TypingDaemon,
                    VoiceDictationMessage::TypingDaemon(UnitSwitchMessage::Press),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(self.handle_face())
    }

    fn handle_face(&self) -> HandleFace<'_> {
        let glyph = Phosphor::Subtitles(Weight::Regular).into();
        let handle = GroupHandle::VoiceDictation;
        match self.dictation.display() {
            Some(reading) => HandleFace::IconColoredBy {
                glyph,
                reading,
                handle,
            },
            None => HandleFace::Icon { glyph, handle },
        }
    }
}
