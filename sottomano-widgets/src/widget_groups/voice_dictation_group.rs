mod notulist_dictation_reading;

use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_sources::{
    SourceDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::{
        notulist_dbus_dictation::{NotulistDictation, send_logged_dictation_command},
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_pipeline_activity::NotulistPipeline,
        notulist_dbus_shortcuts::NotulistShortcuts,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::voice_dictation_group::notulist_dictation_reading::{
        INPUT_METHOD_DAEMON, RECOGNIZER_DAEMON, engine_sections, pipeline_sections,
        shortcut_sections,
    },
    widgets::{
        continuous_reading::ContinuousReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        unit_switch::{UnitSwitch, UnitSwitchMessage},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

const RECOGNIZER_UNIT: &str = "notulist-server.service";
const INPUT_METHOD_UNIT: &str = "notulist-input.service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    Dictation,
    RecognizerDaemon,
    InputMethodDaemon,
}

impl From<VoiceDictationTarget> for HoverTarget {
    fn from(target: VoiceDictationTarget) -> Self {
        HoverTarget::VoiceDictation(target)
    }
}

pub struct VoiceDictationGroup {
    dictation: ContinuousReading<NotulistDictation>,
    shortcuts: ContinuousReading<NotulistShortcuts>,
    engine: ContinuousReading<NotulistEngine>,
    pipeline: ContinuousReading<NotulistPipeline>,
    recognizer_daemon: UnitSwitch,
    input_method_daemon: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VoiceDictationMessage {
    Dictation(SourceDelivery<NotulistDictation>),
    Shortcuts(SourceDelivery<NotulistShortcuts>),
    Engine(SourceDelivery<NotulistEngine>),
    Pipeline(SourceDelivery<NotulistPipeline>),
    ToggleRecording,
    RecognizerDaemon(UnitSwitchMessage),
    InputMethodDaemon(UnitSwitchMessage),
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
            shortcuts: ContinuousReading::default(),
            engine: ContinuousReading::default(),
            pipeline: ContinuousReading::default(),
            recognizer_daemon: UnitSwitch::new(user_unit(RECOGNIZER_UNIT), RECOGNIZER_DAEMON),
            input_method_daemon: UnitSwitch::new(user_unit(INPUT_METHOD_UNIT), INPUT_METHOD_DAEMON),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            source_subscription::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            source_subscription::<NotulistShortcuts>(buses.clone())
                .map(VoiceDictationMessage::Shortcuts),
            source_subscription::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine),
            source_subscription::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            self.recognizer_daemon
                .subscription(buses)
                .map(VoiceDictationMessage::RecognizerDaemon),
            self.input_method_daemon
                .subscription(buses)
                .map(VoiceDictationMessage::InputMethodDaemon),
        ])
    }

    pub fn refresh(
        target: VoiceDictationTarget,
        buses: &DesktopBuses,
    ) -> Task<VoiceDictationMessage> {
        match target {
            VoiceDictationTarget::Dictation
            | VoiceDictationTarget::RecognizerDaemon
            | VoiceDictationTarget::InputMethodDaemon => {
                first_delivery::<NotulistDictation>(buses.clone())
                    .map(VoiceDictationMessage::Dictation)
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
            VoiceDictationMessage::Shortcuts(delivery) => {
                self.shortcuts.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Engine(delivery) => {
                self.engine.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Pipeline(delivery) => {
                self.pipeline.receive_keeping_last_reading(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ToggleRecording => {
                Task::future(send_logged_dictation_command(DictationCommand::Toggle)).discard()
            }
            VoiceDictationMessage::RecognizerDaemon(m) => self
                .recognizer_daemon
                .update(m, clock)
                .map(VoiceDictationMessage::RecognizerDaemon),
            VoiceDictationMessage::InputMethodDaemon(m) => self
                .input_method_daemon
                .update(m, clock)
                .map(VoiceDictationMessage::InputMethodDaemon),
            VoiceDictationMessage::Expansion(m) => {
                self.expansion.update(m);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            VoiceDictationTarget::Dictation => self.dictation.tooltip_with_sections(
                self.pipeline
                    .sections_described_by(pipeline_sections)
                    .into_iter()
                    .chain(self.shortcuts.sections_described_by(shortcut_sections)),
                clock,
            ),
            VoiceDictationTarget::RecognizerDaemon => {
                self.recognizer_daemon.tooltip(true, clock).map(|tooltip| {
                    tooltip.with_sections(self.engine.sections_described_by(engine_sections))
                })
            }
            VoiceDictationTarget::InputMethodDaemon => {
                self.input_method_daemon.tooltip(true, clock)
            }
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
                                notulist_dictation_reading::press_does,
                            ),
                            VoiceDictationMessage::ToggleRecording,
                        )
                    }),
                self.recognizer_daemon.group_item(
                    true,
                    VoiceDictationTarget::RecognizerDaemon,
                    VoiceDictationMessage::RecognizerDaemon(UnitSwitchMessage::Press),
                ),
                self.input_method_daemon.group_item(
                    true,
                    VoiceDictationTarget::InputMethodDaemon,
                    VoiceDictationMessage::InputMethodDaemon(UnitSwitchMessage::Press),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(self.handle_face())
    }

    fn handle_face(&self) -> HandleFace<'_> {
        match self.dictation.display() {
            Some(_) => HandleFace::FirstItem,
            None => HandleFace::Icon {
                glyph: Phosphor::Microphone(Weight::Regular).into(),
                handle: GroupHandle::VoiceDictation,
            },
        }
    }
}
