mod dictation_phase_item;
mod inference_hardware_item;
mod interface_mismatch_item;
mod output_problems_item;
mod pipeline_stage_item;
mod runtime_configuration_item;
mod speech_model_item;
mod speech_runtime_sections;

use std::borrow::Cow;

use iced::{Subscription, Task};
use notulist_client::{DictationCommand, DictationEngine, DictationPhase, EngineState};
use sottomano_desktop_sinks::voice_dictation_input::notulist_dictation_commands::send_logged_dictation_command;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope},
    voice_dictation_input::{
        huggingface_model_card::HuggingFaceModelCard, notulist_dbus_dictation::NotulistDictation,
        notulist_dbus_engine::NotulistEngine,
        notulist_dbus_interface_mismatch::NotulistInterfaceMismatch,
        notulist_dbus_output_problems::NotulistOutputProblems,
        notulist_dbus_pipeline_activity::NotulistPipeline,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        voice_dictation_group::{
            dictation_phase_item::press_does,
            inference_hardware_item::InferenceHardware,
            runtime_configuration_item::RuntimeConfiguration,
            speech_runtime_sections::{SpeechRuntime, idle_runtime_glyph},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFailuresAfterSuccess, LatestSnapshot},
        phosphor::{Phosphor, Weight},
    },
};

const DICTATION_UNIT: &str = "notulist.service";

const SPEECH_RUNTIME: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("Speech runtime"),
    icon: Phosphor::Engine(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "load the speech model and the input method so dictation can start",
        turning_off: "stop dictation, unload the speech model and free the GPU",
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationTarget {
    InterfaceMismatch,
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
    interface_mismatch: LatestSnapshot<NotulistInterfaceMismatch, HidesFailuresAfterSuccess>,
    dictation: LatestSnapshot<NotulistDictation, HidesFailuresAfterSuccess>,
    engine: LatestSnapshot<NotulistEngine, HidesFailuresAfterSuccess>,
    pipeline: LatestSnapshot<NotulistPipeline, HidesFailuresAfterSuccess>,
    output_problems: LatestSnapshot<NotulistOutputProblems, HidesFailuresAfterSuccess>,
    model_card: HoverFetchedSnapshot<HuggingFaceModelCard>,
    runtime: UnitSwitch,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum VoiceDictationMessage {
    InterfaceMismatch(SnapshotDelivery<NotulistInterfaceMismatch>),
    Dictation(SnapshotDelivery<NotulistDictation>),
    Engine(SnapshotDelivery<NotulistEngine>),
    Pipeline(SnapshotDelivery<NotulistPipeline>),
    OutputProblems(SnapshotDelivery<NotulistOutputProblems>),
    ModelCardFetched(SnapshotDelivery<HuggingFaceModelCard>),
    Runtime(UnitSwitchMessage),
    Interaction(VoiceDictationInteraction),
}

#[derive(Debug, Clone)]
pub enum VoiceDictationInteraction {
    ToggleRecording,
    RuntimePressed,
    Expansion(WidgetGroupExpansionMessage),
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
            interface_mismatch: LatestSnapshot::default(),
            dictation: LatestSnapshot::default(),
            engine: LatestSnapshot::default(),
            pipeline: LatestSnapshot::default(),
            output_problems: LatestSnapshot::default(),
            model_card: HoverFetchedSnapshot::default(),
            runtime: UnitSwitch::new(dictation_unit, SPEECH_RUNTIME),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<VoiceDictationMessage> {
        Subscription::batch([
            snapshot_subscription::<NotulistInterfaceMismatch>(buses.clone())
                .map(VoiceDictationMessage::InterfaceMismatch),
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
            VoiceDictationTarget::InterfaceMismatch => {
                first_delivery::<NotulistInterfaceMismatch>(buses.clone())
                    .map(VoiceDictationMessage::InterfaceMismatch)
            }
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
        clock: Clock,
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
            clock,
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
            VoiceDictationMessage::InterfaceMismatch(delivery) => {
                self.interface_mismatch.receive(delivery, clock);
                Task::none()
            }
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
            VoiceDictationMessage::Runtime(message) => self
                .runtime
                .update(message, clock)
                .map(VoiceDictationMessage::Runtime),
            VoiceDictationMessage::Interaction(VoiceDictationInteraction::ToggleRecording) => {
                Task::future(send_logged_dictation_command(DictationCommand::Toggle)).discard()
            }
            VoiceDictationMessage::Interaction(VoiceDictationInteraction::RuntimePressed) => {
                self.runtime.press().map(VoiceDictationMessage::Runtime)
            }
            VoiceDictationMessage::Interaction(VoiceDictationInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
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

    fn runtime_hover_card(&self, clock: Clock) -> Option<HoverCard> {
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
                .hover_card(
                    self.dictation
                        .get()
                        .map_or(ServiceReadiness::NotYetServing, |_| {
                            ServiceReadiness::Serving
                        }),
                    clock,
                )?
                .with_groups(runtime_sections),
        )
    }

    #[must_use]
    pub fn hover_card_for(&self, target: VoiceDictationTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            VoiceDictationTarget::InterfaceMismatch => self.interface_mismatch.hover_card(clock),
            VoiceDictationTarget::Dictation => self.dictation.hover_card(clock),
            VoiceDictationTarget::Pipeline => self.pipeline.hover_card(clock),
            VoiceDictationTarget::Runtime => self.runtime_hover_card(clock),
            VoiceDictationTarget::SpeechModel => self
                .engine
                .hover_card_with_groups(self.model_card.sections(clock), clock),
            VoiceDictationTarget::RuntimeConfiguration => self
                .engine
                .hover_card_with(RuntimeConfiguration(self.loaded_engine()?).hover_card(clock)),
            VoiceDictationTarget::InferenceHardware => self
                .engine
                .hover_card_with(InferenceHardware(self.loaded_engine()?).hover_card(clock)),
            VoiceDictationTarget::TypingOutput => self.output_problems.hover_card(clock),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, VoiceDictationInteraction> {
        let dictation = self.dictation.get();
        let engine = self.loaded_engine();
        let runtime_glyph =
            engine.map_or_else(idle_runtime_glyph, |loaded| SpeechRuntime(loaded).glyph());
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VoiceDictation,
            &self.expansion,
            VoiceDictationInteraction::Expansion,
            [
                self.interface_mismatch
                    .group_item(VoiceDictationTarget::InterfaceMismatch),
                self.dictation
                    .group_item(VoiceDictationTarget::Dictation)
                    .map(|item| {
                        item.on_switch_press(
                            dictation.map_or(SwitchPosition::Off, SwitchPosition::from),
                            dictation.map_or("start dictating at the cursor", press_does),
                            VoiceDictationInteraction::ToggleRecording,
                        )
                        .previewed_on_handle()
                    }),
                self.pipeline
                    .group_item(VoiceDictationTarget::Pipeline)
                    .filter(|_| {
                        self.dictation.get().is_some_and(|dictation| {
                            matches!(
                                dictation.phase,
                                DictationPhase::Listening | DictationPhase::Finishing
                            )
                        }) || self.pipeline.failure().is_some()
                    }),
                self.runtime.group_item_with_glyph(
                    runtime_glyph,
                    dictation.map_or(ServiceReadiness::NotYetServing, |_| {
                        ServiceReadiness::Serving
                    }),
                    VoiceDictationTarget::Runtime,
                    VoiceDictationInteraction::RuntimePressed,
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

    fn handle_face(&self) -> WidgetGroupFace<'_> {
        let glyph = Phosphor::UserSound(Weight::Regular).into();
        match self.dictation.display() {
            Some(reading) => WidgetGroupFace::IconColoredBy { glyph, reading },
            None => WidgetGroupFace::Icon { glyph },
        }
    }
}
