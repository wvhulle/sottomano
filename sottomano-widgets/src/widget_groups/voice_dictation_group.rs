use crate::widget_groups::voice_dictation_group::{
    dictation_phase_indicator::{notulist_dictation_hover_card, notulist_dictation_indicator},
    interface_mismatch_indicator::{
        NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH, notulist_interface_mismatch_hover_card,
        notulist_interface_mismatch_indicator,
    },
    output_problems_indicator::{
        NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH, notulist_output_problems_hover_card,
        notulist_output_problems_indicator,
    },
    pipeline_stage_indicator::{
        NOTULIST_PIPELINE_FALLBACK_GLYPH, notulist_pipeline_hover_card, notulist_pipeline_indicator,
    },
    speech_model_indicator::{
        NOTULIST_ENGINE_FALLBACK_GLYPH, notulist_engine_hover_card, notulist_engine_indicator,
    },
};
mod dictation_phase_indicator;
mod inference_hardware_indicator;
mod interface_mismatch_indicator;
mod output_problems_indicator;
mod pipeline_stage_indicator;
mod runtime_configuration_indicator;
mod speech_model_indicator;
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
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        toggle_control::{SwitchPosition, SwitchPresses},
        unit_switch::{ServiceReadiness, UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    hover_cards::hover_card_body::Section,
    widget_groups::{
        group_summary::GroupSummary,
        voice_dictation_group::{
            dictation_phase_indicator::press_does,
            inference_hardware_indicator::InferenceHardware,
            runtime_configuration_indicator::RuntimeConfiguration,
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

const DICTATION_GLYPH: Phosphor = Phosphor::UserSound(Weight::Regular);
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
            .sections_described_by(|sample| match &sample.engine {
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

    fn dictation_indicator<'a>(
        &'a self,
        dictation_row: GroupIndicator<'a>,
        switch_position: SwitchPosition,
        press_text: &'static str,
    ) -> GroupIndicator<'a> {
        dictation_row
            .on_switch_press(
                switch_position,
                press_text,
                VoiceDictationInteraction::ToggleRecording,
            )
            .with_hover_card(Box::new(move |_| {
                self.dictation
                    .sections_described_by(notulist_dictation_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Dictation))
    }

    fn interface_mismatch_indicator<'a>(
        &'a self,
        mismatch_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        mismatch_indicator
            .with_hover_card(Box::new(move |_| {
                self.interface_mismatch
                    .sections_described_by(notulist_interface_mismatch_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(
                VoiceDictationSource::InterfaceMismatch,
            ))
    }

    fn pipeline_indicator<'a>(&'a self, pipeline_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        pipeline_row
            .with_hover_card(Box::new(move |_| {
                self.pipeline
                    .sections_described_by(notulist_pipeline_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Pipeline))
    }

    fn engine_indicator<'a>(&'a self, engine_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        let engine_indicator = engine_row
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .engine
                    .sections_described_by(notulist_engine_hover_card);
                sections.extend(
                    self.model_card
                        .sections_described_by(hugging_face_model_card_sections),
                );
                sections
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine));
        match self.model_repository() {
            Some(repository) => engine_indicator.fetching_on_hover(
                VoiceDictationInteraction::StartModelCardFetch(repository),
                VoiceDictationInteraction::StopHoverFetches,
            ),
            None => engine_indicator,
        }
    }

    fn output_problems_indicator<'a>(
        &'a self,
        problems_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        problems_indicator
            .with_hover_card(Box::new(move |_| {
                self.output_problems
                    .sections_described_by(notulist_output_problems_hover_card)
            }))
            .refreshed_by(Self::refreshed_from(VoiceDictationSource::OutputProblems))
    }

    fn dictation_summary(&self, clock: Clock) -> GroupSummary<'_> {
        match &self.dictation {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                Phosphor::ChatTeardropText(Weight::Regular).into(),
                self.dictation_indicator(
                    notulist_dictation_indicator(held_snapshot),
                    SwitchPosition::from(held_snapshot),
                    press_does(held_snapshot),
                ),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let held_snapshot = &troubled_snapshot.held_snapshot;
                GroupSummary::summarising(
                    Phosphor::ChatTeardropText(Weight::Regular).into(),
                    self.dictation_indicator(
                        troubled_snapshot
                            .shown_over(notulist_dictation_indicator(held_snapshot), clock),
                        SwitchPosition::from(held_snapshot),
                        press_does(held_snapshot),
                    ),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                Phosphor::ChatTeardropText(Weight::Regular).into(),
                self.dictation_indicator(
                    placeholder.indicator(DICTATION_GLYPH.into()),
                    SwitchPosition::Off,
                    "start dictating at the cursor",
                ),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupSummary::glyph(
                Phosphor::ChatTeardropText(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupSummary::glyph(
                Phosphor::ChatTeardropText(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        }
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = self.dictation_summary(clock);
        let mut rows = Vec::new();
        self.push_interface_mismatch_indicators(&mut rows, clock);
        self.push_pipeline_indicators(&mut rows, clock);
        self.push_runtime_switch_indicators(&mut rows);
        self.push_engine_indicators(&mut rows, clock);
        self.push_loaded_engine_indicators(&mut rows);
        self.push_output_problems_indicators(&mut rows, clock);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::VoiceDictation,
            group_summary,
            &self.expansion,
            VoiceDictationInteraction::Expansion,
            rows,
        )
    }

    fn push_interface_mismatch_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
    ) {
        match &self.interface_mismatch {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let mismatch_indicator = notulist_interface_mismatch_indicator(held_snapshot);
                if mismatch_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.interface_mismatch_indicator(mismatch_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let mismatch_indicator = troubled_snapshot.shown_over(
                    notulist_interface_mismatch_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if mismatch_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.interface_mismatch_indicator(mismatch_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.interface_mismatch_indicator(
                    placeholder.indicator(NOTULIST_INTERFACE_MISMATCH_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_pipeline_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        let dictation = self.dictation.get();
        let pipeline_active = dictation.is_some_and(|held_dictation| {
            matches!(
                held_dictation.phase,
                DictationPhase::Listening | DictationPhase::Finishing
            )
        });
        if pipeline_active || self.pipeline.failure().is_some() {
            match &self.pipeline {
                SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                    let pipeline_row = notulist_pipeline_indicator(held_snapshot);
                    if pipeline_row.panel_visibility() == PanelVisibility::Shown {
                        rows.push(self.pipeline_indicator(pipeline_row));
                    }
                }
                SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                    let pipeline_row = troubled_snapshot.shown_over(
                        notulist_pipeline_indicator(&troubled_snapshot.held_snapshot),
                        clock,
                    );
                    if pipeline_row.panel_visibility() == PanelVisibility::Shown {
                        rows.push(self.pipeline_indicator(pipeline_row));
                    }
                }
                SubscribedSnapshot::NoSnapshot { placeholder } => {
                    rows.push(self.pipeline_indicator(
                        placeholder.indicator(NOTULIST_PIPELINE_FALLBACK_GLYPH),
                    ));
                }
                SubscribedSnapshot::AwaitingFirstSnapshot
                | SubscribedSnapshot::SourceUnavailable => {}
            }
        }
    }

    fn push_runtime_switch_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        let dictation = self.dictation.get();
        let runtime_glyph = self
            .loaded_engine()
            .map_or_else(idle_runtime_glyph, |loaded| SpeechRuntime(loaded).glyph());
        let runtime_readiness = dictation.map_or(ServiceReadiness::NotYetServing, |_| {
            ServiceReadiness::Serving
        });
        if let Some(switch_indicator) = self.runtime.switch_indicator(runtime_readiness) {
            rows.push(
                switch_indicator
                    .with_glyph(runtime_glyph)
                    .wired_switch_indicator(VoiceDictationInteraction::RuntimePressed)
                    .with_hover_card(Box::new(move |_| self.runtime_hover_card()))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
        }
    }

    fn push_engine_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.engine {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let engine_row = notulist_engine_indicator(held_snapshot);
                if engine_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.engine_indicator(engine_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let engine_row = troubled_snapshot.shown_over(
                    notulist_engine_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if engine_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.engine_indicator(engine_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.engine_indicator(placeholder.indicator(NOTULIST_ENGINE_FALLBACK_GLYPH)),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_loaded_engine_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(loaded) = self.loaded_engine() {
            rows.push(
                RuntimeConfiguration(loaded)
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(RuntimeConfiguration(loaded).hover_card())
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
            rows.push(
                InferenceHardware(loaded)
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.engine
                            .hover_card_with(InferenceHardware(loaded).hover_card())
                    }))
                    .refreshed_by(Self::refreshed_from(VoiceDictationSource::Engine)),
            );
        }
    }

    fn push_output_problems_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
    ) {
        match &self.output_problems {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let problems_indicator = notulist_output_problems_indicator(held_snapshot);
                if problems_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.output_problems_indicator(problems_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let problems_indicator = troubled_snapshot.shown_over(
                    notulist_output_problems_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if problems_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.output_problems_indicator(problems_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.output_problems_indicator(
                    placeholder.indicator(NOTULIST_OUTPUT_PROBLEMS_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }
}
