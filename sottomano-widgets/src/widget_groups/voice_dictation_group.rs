mod compute_device_item;
mod dictation_phase_item;
mod output_problems_item;
mod pipeline_stage_item;
mod speech_model_item;

use iced::{Subscription, Task};
use notulist_client::{DictationCommand, DictationEngine, EngineState};
use sottomano_sources::{
    SourceDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::{
        notulist_dbus_dictation::{NotulistDictation, send_logged_dictation_command},
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_output_problems::NotulistOutputProblems,
        notulist_dbus_pipeline_activity::NotulistPipeline,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::voice_dictation_group::{
        compute_device_item::ComputeDevice,
        dictation_phase_item::{press_does, shows_pipeline},
    },
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::Tooltip,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

const DICTATION_UNIT: &str = "notulist.service";

const DICTATION_DAEMON: UnitSwitchFace = UnitSwitchFace {
    name: "Dictation daemon",
    icon: Phosphor::Brain(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model and the input method so dictation can start",
        turning_off: "stop dictation, unload the speech model and free the GPU",
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    Dictation,
    Pipeline,
    SpeechModel,
    Compute,
    TypingOutput,
    Daemon,
}

impl From<VoiceDictationTarget> for HoverTarget {
    fn from(target: VoiceDictationTarget) -> Self {
        HoverTarget::VoiceDictation(target)
    }
}

pub struct VoiceDictationGroup {
    dictation: ContinuousReading<NotulistDictation>,
    engine: ContinuousReading<NotulistEngine>,
    pipeline: ContinuousReading<NotulistPipeline>,
    output_problems: ContinuousReading<NotulistOutputProblems>,
    daemon: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VoiceDictationMessage {
    Dictation(SourceDelivery<NotulistDictation>),
    Engine(SourceDelivery<NotulistEngine>),
    Pipeline(SourceDelivery<NotulistPipeline>),
    OutputProblems(SourceDelivery<NotulistOutputProblems>),
    ToggleRecording,
    Daemon(UnitSwitchMessage),
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
        let dictation_unit = ScopedSystemdUnit {
            scope: SystemdUnitScope::UserSession,
            unit_name: DICTATION_UNIT.to_owned(),
        };
        Self {
            dictation: ContinuousReading::default(),
            engine: ContinuousReading::default(),
            pipeline: ContinuousReading::default(),
            output_problems: ContinuousReading::default(),
            daemon: UnitSwitch::new(dictation_unit, DICTATION_DAEMON),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            source_subscription::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            source_subscription::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine),
            source_subscription::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            source_subscription::<NotulistOutputProblems>(buses.clone())
                .map(VoiceDictationMessage::OutputProblems),
            self.daemon
                .subscription(buses)
                .map(VoiceDictationMessage::Daemon),
        ])
    }

    pub fn refresh(
        target: VoiceDictationTarget,
        buses: &DesktopBuses,
    ) -> Task<VoiceDictationMessage> {
        match target {
            VoiceDictationTarget::Dictation | VoiceDictationTarget::Daemon => {
                first_delivery::<NotulistDictation>(buses.clone())
                    .map(VoiceDictationMessage::Dictation)
            }
            VoiceDictationTarget::Pipeline => first_delivery::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            VoiceDictationTarget::SpeechModel | VoiceDictationTarget::Compute => {
                first_delivery::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine)
            }
            VoiceDictationTarget::TypingOutput => {
                first_delivery::<NotulistOutputProblems>(buses.clone())
                    .map(VoiceDictationMessage::OutputProblems)
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
                self.dictation.receive_keeping_last_success(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Engine(delivery) => {
                self.engine.receive_keeping_last_success(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Pipeline(delivery) => {
                self.pipeline.receive_keeping_last_success(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::OutputProblems(delivery) => {
                self.output_problems
                    .receive_keeping_last_success(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ToggleRecording => {
                Task::future(send_logged_dictation_command(DictationCommand::Toggle)).discard()
            }
            VoiceDictationMessage::Daemon(m) => self
                .daemon
                .update(m, clock)
                .map(VoiceDictationMessage::Daemon),
            VoiceDictationMessage::Expansion(m) => {
                self.expansion.update(m);
                Task::none()
            }
        }
    }

    fn loaded_engine(&self) -> Option<&DictationEngine> {
        match &self.engine.get()?.engine {
            EngineState::SpeechModelReady(engine) => Some(engine),
            EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => None,
        }
    }

    fn dictating(&self) -> bool {
        self.dictation.get().is_some_and(shows_pipeline)
    }

    #[must_use]
    pub fn tooltip_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            VoiceDictationTarget::Dictation => self.dictation.tooltip(clock),
            VoiceDictationTarget::Pipeline => self.pipeline.tooltip(clock),
            VoiceDictationTarget::SpeechModel => self.engine.tooltip(clock),
            VoiceDictationTarget::Compute => self
                .engine
                .tooltip_with(ComputeDevice(self.loaded_engine()?).tooltip(clock)),
            VoiceDictationTarget::TypingOutput => self.output_problems.tooltip(clock),
            VoiceDictationTarget::Daemon => self.daemon.tooltip(true, clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, VoiceDictationMessage> {
        let dictation = self.dictation.get();
        let engine = self.loaded_engine();
        GroupContent::new(
            &self.expansion,
            VoiceDictationMessage::Expansion,
            [
                self.dictation
                    .group_item(VoiceDictationTarget::Dictation)
                    .map(|item| {
                        item.on_press(
                            dictation.map_or("start dictating at the cursor", press_does),
                            VoiceDictationMessage::ToggleRecording,
                        )
                        .previewed_on_handle()
                    }),
                self.pipeline
                    .group_item(VoiceDictationTarget::Pipeline)
                    .filter(|_| self.dictating() || self.pipeline.failure().is_some()),
                self.engine.group_item(VoiceDictationTarget::SpeechModel),
                engine.map(|engine| {
                    GroupItem::reading(ComputeDevice(engine), VoiceDictationTarget::Compute)
                }),
                self.output_problems
                    .group_item(VoiceDictationTarget::TypingOutput),
                self.daemon.group_item(
                    self.dictation.get().is_some(),
                    VoiceDictationTarget::Daemon,
                    VoiceDictationMessage::Daemon(UnitSwitchMessage::Press),
                ),
            ]
            .into_iter()
            .flatten(),
        )
        .face(self.handle_face())
    }

    fn handle_face(&self) -> HandleFace<'_> {
        let glyph = Phosphor::UserSound(Weight::Regular).into();
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
