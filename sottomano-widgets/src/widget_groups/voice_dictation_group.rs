mod dictation_phase_item;
mod inference_hardware_item;
mod output_problems_item;
mod pipeline_stage_item;
mod runtime_configuration_item;
mod speech_model_item;
mod speech_runtime_sections;

use iced::{Subscription, Task};
use notulist_client::{DictationCommand, DictationEngine, EngineState};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::{
        huggingface_model_card::HuggingFaceModelCard,
        notulist_dbus_dictation::{NotulistDictation, send_logged_dictation_command},
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_output_problems::NotulistOutputProblems,
        notulist_dbus_pipeline_activity::NotulistPipeline,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::voice_dictation_group::{
        dictation_phase_item::{press_does, shows_pipeline},
        inference_hardware_item::InferenceHardware,
        runtime_configuration_item::RuntimeConfiguration,
        speech_runtime_sections::{SpeechRuntime, idle_runtime_glyph},
    },
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFailuresAfterSuccess, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::Tooltip,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

const DICTATION_UNIT: &str = "notulist.service";

const SPEECH_RUNTIME: UnitSwitchFace = UnitSwitchFace {
    name: "Speech runtime",
    icon: Phosphor::Engine(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model and the input method so dictation can start",
        turning_off: "stop dictation, unload the speech model and free the GPU",
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    Dictation,
    Pipeline,
    Runtime,
    SpeechModel,
    RuntimeConfiguration,
    InferenceHardware,
    TypingOutput,
}

impl From<VoiceDictationTarget> for HoverTarget {
    fn from(target: VoiceDictationTarget) -> Self {
        HoverTarget::VoiceDictation(target)
    }
}

pub struct VoiceDictationGroup {
    dictation: LatestSnapshot<NotulistDictation, HidesFailuresAfterSuccess>,
    engine: LatestSnapshot<NotulistEngine, HidesFailuresAfterSuccess>,
    pipeline: LatestSnapshot<NotulistPipeline, HidesFailuresAfterSuccess>,
    output_problems: LatestSnapshot<NotulistOutputProblems, HidesFailuresAfterSuccess>,
    model_card: HoverFetchedSnapshot<HuggingFaceModelCard>,
    runtime: UnitSwitch,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum VoiceDictationMessage {
    Dictation(SnapshotDelivery<NotulistDictation>),
    Engine(SnapshotDelivery<NotulistEngine>),
    Pipeline(SnapshotDelivery<NotulistPipeline>),
    OutputProblems(SnapshotDelivery<NotulistOutputProblems>),
    ModelCardFetched(SnapshotDelivery<HuggingFaceModelCard>),
    ToggleRecording,
    Runtime(UnitSwitchMessage),
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
            dictation: LatestSnapshot::default(),
            engine: LatestSnapshot::default(),
            pipeline: LatestSnapshot::default(),
            output_problems: LatestSnapshot::default(),
            model_card: HoverFetchedSnapshot::default(),
            runtime: UnitSwitch::new(dictation_unit, SPEECH_RUNTIME),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            snapshot_subscription::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            snapshot_subscription::<NotulistEngine>(buses.clone())
                .map(VoiceDictationMessage::Engine),
            snapshot_subscription::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            snapshot_subscription::<NotulistOutputProblems>(buses.clone())
                .map(VoiceDictationMessage::OutputProblems),
            self.runtime
                .subscription(buses)
                .map(VoiceDictationMessage::Runtime),
        ])
    }

    pub fn refresh(
        target: VoiceDictationTarget,
        buses: &DesktopBuses,
    ) -> Task<VoiceDictationMessage> {
        match target {
            VoiceDictationTarget::Dictation => first_delivery::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            VoiceDictationTarget::Pipeline => first_delivery::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            VoiceDictationTarget::Runtime
            | VoiceDictationTarget::SpeechModel
            | VoiceDictationTarget::RuntimeConfiguration
            | VoiceDictationTarget::InferenceHardware => {
                first_delivery::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine)
            }
            VoiceDictationTarget::TypingOutput => {
                first_delivery::<NotulistOutputProblems>(buses.clone())
                    .map(VoiceDictationMessage::OutputProblems)
            }
        }
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<VoiceDictationMessage> {
        self.model_card.stop();
        let Some(HoverTarget::VoiceDictation(VoiceDictationTarget::SpeechModel)) = hovered else {
            return Task::none();
        };
        let Some(repository) = self
            .loaded_engine()
            .map(|engine| engine.model_repository.clone())
            .filter(|repository| !repository.is_empty())
        else {
            return Task::none();
        };
        self.model_card.start_hover_fetch(
            repository,
            VoiceDictationMessage::ModelCardFetched,
            config,
        )
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
                self.dictation.receive(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Engine(delivery) => {
                self.engine.receive(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::Pipeline(delivery) => {
                self.pipeline.receive(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::OutputProblems(delivery) => {
                self.output_problems.receive(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ModelCardFetched(delivery) => {
                self.model_card.receive(delivery, clock);
                Task::none()
            }
            VoiceDictationMessage::ToggleRecording => {
                Task::future(send_logged_dictation_command(DictationCommand::Toggle)).discard()
            }
            VoiceDictationMessage::Runtime(message) => self
                .runtime
                .update(message, clock)
                .map(VoiceDictationMessage::Runtime),
            VoiceDictationMessage::Expansion(message) => {
                self.expansion.update(message);
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

    fn runtime_tooltip(&self, clock: Clock) -> Option<Tooltip> {
        let serving = self.dictation.get().is_some();
        let runtime_sections = self
            .engine
            .sections_described_by(|reading| match &reading.engine {
                EngineState::SpeechModelReady(engine) => SpeechRuntime(engine).sections(),
                EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => {
                    Vec::new()
                }
            });
        Some(
            self.runtime
                .tooltip(serving, clock)?
                .with_groups(runtime_sections),
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            VoiceDictationTarget::Dictation => self.dictation.tooltip(clock),
            VoiceDictationTarget::Pipeline => self.pipeline.tooltip(clock),
            VoiceDictationTarget::Runtime => self.runtime_tooltip(clock),
            VoiceDictationTarget::SpeechModel => self
                .engine
                .tooltip_with_groups(self.model_card.sections(clock), clock),
            VoiceDictationTarget::RuntimeConfiguration => self
                .engine
                .tooltip_with(RuntimeConfiguration(self.loaded_engine()?).tooltip(clock)),
            VoiceDictationTarget::InferenceHardware => self
                .engine
                .tooltip_with(InferenceHardware(self.loaded_engine()?).tooltip(clock)),
            VoiceDictationTarget::TypingOutput => self.output_problems.tooltip(clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, VoiceDictationMessage> {
        let dictation = self.dictation.get();
        let engine = self.loaded_engine();
        let runtime_glyph =
            engine.map_or_else(idle_runtime_glyph, |loaded| SpeechRuntime(loaded).glyph());
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
                self.runtime.group_item_with_glyph(
                    runtime_glyph,
                    dictation.is_some(),
                    VoiceDictationTarget::Runtime,
                    VoiceDictationMessage::Runtime(UnitSwitchMessage::Press),
                ),
                self.engine.group_item(VoiceDictationTarget::SpeechModel),
                engine.map(|loaded| {
                    GroupItem::reading(
                        RuntimeConfiguration(loaded),
                        VoiceDictationTarget::RuntimeConfiguration,
                    )
                }),
                engine.map(|loaded| {
                    GroupItem::reading(
                        InferenceHardware(loaded),
                        VoiceDictationTarget::InferenceHardware,
                    )
                }),
                self.output_problems
                    .group_item(VoiceDictationTarget::TypingOutput),
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
