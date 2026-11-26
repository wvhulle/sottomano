mod dictation_indicator_rows;
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
use notulist_client::DictationCommand;
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
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        toggle_control::SwitchPresses,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
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
    expansion: IndicatorGroupExpansion,
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
    Expansion(IndicatorGroupExpansionMessage),
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
            expansion: IndicatorGroupExpansion::default(),
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

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.dictation_headline();
        let mut rows = Vec::new();
        self.push_interface_mismatch_indicators(&mut rows);
        self.push_pipeline_indicators(&mut rows);
        self.push_runtime_switch_indicators(&mut rows);
        self.push_engine_indicators(&mut rows);
        self.push_loaded_engine_indicators(&mut rows);
        self.push_output_problems_indicators(&mut rows);
        self.list_dictation(&mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::VoiceDictation,
            headline_indicator,
            &self.expansion,
            VoiceDictationInteraction::Expansion,
            IndicatorListing::Listed(rows),
        )
    }
}
