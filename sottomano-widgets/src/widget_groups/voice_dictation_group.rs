use crate::widget_groups::voice_dictation_group::{
    dictation_phase_item::{notulist_dictation_hover_card, notulist_dictation_reading_row},
    interface_mismatch_item::{
        NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH, notulist_interface_mismatch_hover_card,
        notulist_interface_mismatch_reading_row,
    },
    output_problems_item::{
        NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH, notulist_output_problems_hover_card,
        notulist_output_problems_reading_row,
    },
    pipeline_stage_item::{
        NOTULIST_PIPELINE_FALLBACK_GLYPH, notulist_pipeline_hover_card,
        notulist_pipeline_reading_row,
    },
    speech_model_item::{
        NOTULIST_ENGINE_FALLBACK_GLYPH, notulist_engine_hover_card, notulist_engine_reading_row,
    },
};
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
use sottomano_desktop_sinks::voice_dictation_input::notulist_dictation_commands::send_dictation_command;
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
    background_work::{first_delivery, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        voice_dictation_group::{
            dictation_phase_item::press_does,
            inference_hardware_item::InferenceHardware,
            runtime_configuration_item::RuntimeConfiguration,
            speech_runtime_sections::{SpeechRuntime, idle_runtime_glyph},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        huggingface_hub_sections::hugging_face_model_card_sections,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
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
pub enum VoiceDictationSource {
    InterfaceMismatch,
    Dictation,
    Pipeline,
    Engine,
    OutputProblems,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceDictationBusRequest {
    Refresh(VoiceDictationSource),
}

pub struct VoiceDictationGroup {
    interface_mismatch: SubscribedSnapshot<NotulistInterfaceMismatch>,
    dictation: SubscribedSnapshot<NotulistDictation>,
    engine: SubscribedSnapshot<NotulistEngine>,
    pipeline: SubscribedSnapshot<NotulistPipeline>,
    output_problems: SubscribedSnapshot<NotulistOutputProblems>,
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
}

#[derive(Debug, Clone)]
pub enum VoiceDictationInteraction {
    ToggleRecording,
    RuntimePressed,
    Expansion(WidgetGroupExpansionMessage),
    StartModelCardFetch(String),
    StopHoverFetches,
    OnBuses(VoiceDictationBusRequest),
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
            interface_mismatch: SubscribedSnapshot::default(),
            dictation: SubscribedSnapshot::default(),
            engine: SubscribedSnapshot::default(),
            pipeline: SubscribedSnapshot::default(),
            output_problems: SubscribedSnapshot::default(),
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

    pub fn on_buses(
        request: VoiceDictationBusRequest,
        buses: &DesktopBuses,
    ) -> Task<VoiceDictationMessage> {
        let VoiceDictationBusRequest::Refresh(source) = request;
        match source {
            VoiceDictationSource::InterfaceMismatch => {
                first_delivery::<NotulistInterfaceMismatch>(buses.clone())
                    .map(VoiceDictationMessage::InterfaceMismatch)
            }
            VoiceDictationSource::Dictation => first_delivery::<NotulistDictation>(buses.clone())
                .map(VoiceDictationMessage::Dictation),
            VoiceDictationSource::Pipeline => first_delivery::<NotulistPipeline>(buses.clone())
                .map(VoiceDictationMessage::Pipeline),
            VoiceDictationSource::Engine => {
                first_delivery::<NotulistEngine>(buses.clone()).map(VoiceDictationMessage::Engine)
            }
            VoiceDictationSource::OutputProblems => {
                first_delivery::<NotulistOutputProblems>(buses.clone())
                    .map(VoiceDictationMessage::OutputProblems)
            }
        }
    }

    fn refreshed_from(source: VoiceDictationSource) -> VoiceDictationInteraction {
        VoiceDictationInteraction::OnBuses(VoiceDictationBusRequest::Refresh(source))
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
        }
    }

    pub fn interact(
        &mut self,
        interaction: VoiceDictationInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<VoiceDictationMessage> {
        match interaction {
            VoiceDictationInteraction::ToggleRecording => logged_failure_task(
                "notulist toggle recording".to_owned(),
                send_dictation_command(DictationCommand::Toggle),
            ),
            VoiceDictationInteraction::RuntimePressed => {
                self.runtime.press().map(VoiceDictationMessage::Runtime)
            }
            VoiceDictationInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            VoiceDictationInteraction::StartModelCardFetch(repository) => {
                self.model_card.start_hover_fetch(
                    repository,
                    VoiceDictationMessage::ModelCardFetched,
                    config,
                    clock,
                )
            }
            VoiceDictationInteraction::StopHoverFetches => {
                self.model_card.stop();
                Task::none()
            }
            VoiceDictationInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn model_repository(&self) -> Option<String> {
        self.loaded_engine()
            .map(|engine| engine.model_repository.clone())
            .filter(|repository| !repository.is_empty())
    }

    fn loaded_engine(&self) -> Option<&DictationEngine> {
        match &self.engine.get()?.engine {
            EngineState::SpeechModelReady(engine) => Some(engine),
            EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => None,
        }
    }

    fn runtime_hover_card(&self) -> Vec<Section> {
        let runtime_sections = self
            .engine
            .sections_described_by(|reading| match &reading.engine {
                EngineState::SpeechModelReady(engine) => SpeechRuntime(engine).sections(),
                EngineState::LoadingSpeechModel | EngineState::SpeechModelUnavailable(_) => {
                    Vec::new()
                }
            });
        let mut sections = self.runtime.hover_card(
            self.dictation
                .get()
                .map_or(ServiceReadiness::NotYetServing, |_| {
                    ServiceReadiness::Serving
                }),
        );
        sections.extend(runtime_sections);
        sections
    }

    fn engine_items(&self, clock: Clock) -> [Option<GroupItem<'_, VoiceDictationInteraction>>; 4] {
        let engine = self.loaded_engine();
        [
            self.engine
                .group_item(
                    NOTULIST_ENGINE_FALLBACK_GLYPH,
                    |snapshot| notulist_engine_reading_row(snapshot),
                    clock,
                )
                .map(|item| {
                    let engine_item = item
                        .with_hover_card(Box::new(move |_| {
                            let mut sections = self
                                .engine
                                .sections_described_by(notulist_engine_hover_card);
                            sections.extend(self.model_card.sections_described_by(|snapshot| {
                                hugging_face_model_card_sections(snapshot)
                            }));
                            sections
                        }))
                        .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine));
                    match self.model_repository() {
                        Some(repository) => engine_item.fetching_on_hover(
                            VoiceDictationInteraction::StartModelCardFetch(repository),
                            VoiceDictationInteraction::StopHoverFetches,
                        ),
                        None => engine_item,
                    }
                }),
            engine.map(|loaded| {
                GroupItem::reading(RuntimeConfiguration(loaded).reading_row())
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(RuntimeConfiguration(loaded).hover_card())
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine))
            }),
            engine.map(|loaded| {
                GroupItem::reading(InferenceHardware(loaded).reading_row())
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(InferenceHardware(loaded).hover_card())
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine))
            }),
            self.output_problems
                .group_item(
                    NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH,
                    |snapshot| notulist_output_problems_reading_row(snapshot),
                    clock,
                )
                .map(|item| {
                    item.with_hover_card(Box::new(move |_| {
                        self.output_problems
                            .sections_described_by(notulist_output_problems_hover_card)
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::OutputProblems))
                }),
        ]
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_, VoiceDictationInteraction> {
        let dictation = self.dictation.get();
        let engine = self.loaded_engine();
        let runtime_glyph =
            engine.map_or_else(idle_runtime_glyph, |loaded| SpeechRuntime(loaded).glyph());
        let dictation_item = GroupItem::reading(self.dictation.main_row(
            Phosphor::UserSound(Weight::Regular).into(),
            notulist_dictation_reading_row,
            clock,
        ))
        .on_switch_press(
            dictation.map_or(SwitchPosition::Off, SwitchPosition::from),
            dictation.map_or("start dictating at the cursor", press_does),
            VoiceDictationInteraction::ToggleRecording,
        )
        .with_hover_card(Box::new(move |_| {
            self.dictation
                .sections_described_by(notulist_dictation_hover_card)
        }))
        .refreshed_by(Self::refreshed_from(VoiceDictationSource::Dictation));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VoiceDictation,
            GroupSummary::showing(Phosphor::UserSound(Weight::Regular).into(), &dictation_item),
            &self.expansion,
            VoiceDictationInteraction::Expansion,
            [
                self.interface_mismatch
                    .group_item(
                        NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH,
                        |snapshot| notulist_interface_mismatch_reading_row(snapshot),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.interface_mismatch
                                .sections_described_by(notulist_interface_mismatch_hover_card)
                        }))
                        .refreshed_by(Self::refreshed_from(
                            VoiceDictationSource::InterfaceMismatch,
                        ))
                    }),
                Some(dictation_item),
                self.pipeline
                    .group_item(
                        NOTULIST_PIPELINE_FALLBACK_GLYPH,
                        |snapshot| notulist_pipeline_reading_row(snapshot),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.pipeline
                                .sections_described_by(notulist_pipeline_hover_card)
                        }))
                        .refreshed_by(Self::refreshed_from(VoiceDictationSource::Pipeline))
                    })
                    .filter(|_| {
                        self.dictation.get().is_some_and(|dictation| {
                            matches!(
                                dictation.phase,
                                DictationPhase::Listening | DictationPhase::Finishing
                            )
                        }) || self.pipeline.failure().is_some()
                    }),
                self.runtime
                    .group_item_with_glyph(
                        runtime_glyph,
                        dictation.map_or(ServiceReadiness::NotYetServing, |_| {
                            ServiceReadiness::Serving
                        }),
                        VoiceDictationInteraction::RuntimePressed,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| self.runtime_hover_card()))
                            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine))
                    }),
            ]
            .into_iter()
            .flatten()
            .chain(self.engine_items(clock).into_iter().flatten()),
        )
    }
}
