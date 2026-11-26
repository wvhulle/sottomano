use crate::{
    group_indicators::indicator_state::IndicatorState,
    indicator_groups::indicator_group_content::IndicatorListing,
};
mod context_fill_gauge;
mod generation_rate_label;
mod gguf_model_card_sections;
mod loaded_model_label;
mod scoped_servers;
mod slot_occupancy_gauge;

use std::borrow::Cow;

use iced::{Subscription, Task};
use scoped_servers::{ScopedLlamaServers, ScopedServersInteraction, ScopedServersMessage};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        gguf_header_model_card::GgufModelCard,
        llama_server_slots::{ChildInstance, LlamaModelProps, ModelFileArgument},
    },
    system_protocol_clients::dbus_desktop_buses::{Bus, DesktopBuses},
    systemd_units_journal::SystemdUnitScope,
    voice_dictation_input::huggingface_model_card::HuggingFaceModelCard,
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, toggle_control::SwitchPresses, unit_switch::UnitSwitchFace,
    },
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::hover_fetched_snapshot::{HoverFetchedSnapshot, SnapshotFetchStage},
};

const SERVER_SWITCH: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("llama-server"),
    icon: Phosphor::PlugsConnected(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start llama-server; models load on their first request",
        turning_off: "stop llama-server and free the memory its models hold",
    },
};
use crate::{
    indicator_groups::llama_server_group::{
        gguf_model_card_sections::gguf_model_card_sections,
        loaded_model_label::llama_model_props_sections,
    },
    widgets::huggingface_hub_sections::hugging_face_model_card_sections,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedModelUnit {
    pub(crate) bus: Bus,
    pub(crate) unit: String,
}

pub struct LlamaServerGroup {
    user: ScopedLlamaServers,
    system: ScopedLlamaServers,
    model_props: HoverFetchedSnapshot<LlamaModelProps>,
    gguf_card: HoverFetchedSnapshot<GgufModelCard>,
    base_model_card: HoverFetchedSnapshot<HuggingFaceModelCard>,
    expansion: IndicatorGroupExpansion,
}

impl Default for LlamaServerGroup {
    fn default() -> Self {
        Self {
            user: ScopedLlamaServers::new(SystemdUnitScope::UserSession),
            system: ScopedLlamaServers::new(SystemdUnitScope::SystemWide),
            model_props: HoverFetchedSnapshot::default(),
            gguf_card: HoverFetchedSnapshot::default(),
            base_model_card: HoverFetchedSnapshot::default(),
            expansion: IndicatorGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum LlamaServerMessage {
    User(ScopedServersMessage),
    System(ScopedServersMessage),
    ModelPropsFetched(SnapshotDelivery<LlamaModelProps>),
    GgufCardRead(SnapshotDelivery<GgufModelCard>),
    BaseModelCardFetched(SnapshotDelivery<HuggingFaceModelCard>),
}

#[derive(Debug, Clone)]
pub enum LlamaServerInteraction {
    User(ScopedServersInteraction),
    System(ScopedServersInteraction),
    Expansion(IndicatorGroupExpansionMessage),
    StartModelFetches(ServedModelUnit),
    StopHoverFetches,
}

impl LlamaServerGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<LlamaServerMessage> {
        Subscription::batch([
            self.user.subscription(buses).map(LlamaServerMessage::User),
            self.system
                .subscription(buses)
                .map(LlamaServerMessage::System),
        ])
    }

    fn stop_hover_fetches(&mut self) {
        self.model_props.stop();
        self.gguf_card.stop();
        self.base_model_card.stop();
    }

    fn start_model_fetches(
        &mut self,
        served_model: &ServedModelUnit,
        config: &Config,
        clock: Clock,
    ) -> Task<LlamaServerMessage> {
        let servers = match served_model.bus {
            Bus::Session => &self.user,
            Bus::System => &self.system,
        };
        let Some(resident_model) = servers.resident_model(&served_model.unit).cloned() else {
            return Task::none();
        };
        let mut model_fetches = Vec::new();
        if let ChildInstance::Listening(instance) = resident_model.instance {
            model_fetches.push(self.model_props.start_hover_fetch(
                instance,
                LlamaServerMessage::ModelPropsFetched,
                config,
                clock,
            ));
        }
        if let ModelFileArgument::Given(model_file) = resident_model.model_file {
            model_fetches.push(self.gguf_card.start_hover_fetch(
                model_file,
                LlamaServerMessage::GgufCardRead,
                config,
                clock,
            ));
        }
        Task::batch(model_fetches)
    }

    fn fetch_base_model_card(&mut self, config: &Config, clock: Clock) -> Task<LlamaServerMessage> {
        if matches!(self.gguf_card.hover_fetch, SnapshotFetchStage::Idle) {
            return Task::none();
        }
        let Some(repository) = self
            .gguf_card
            .sample()
            .and_then(GgufModelCard::base_model_repository)
            .map(str::to_owned)
        else {
            return Task::none();
        };
        self.base_model_card.start_hover_fetch(
            repository,
            LlamaServerMessage::BaseModelCardFetched,
            config,
            clock,
        )
    }

    pub fn update(
        &mut self,
        message: LlamaServerMessage,
        config: &Config,
        clock: Clock,
    ) -> Task<LlamaServerMessage> {
        match message {
            LlamaServerMessage::User(message) => self
                .user
                .update(message, clock)
                .map(LlamaServerMessage::User),
            LlamaServerMessage::System(message) => self
                .system
                .update(message, clock)
                .map(LlamaServerMessage::System),
            LlamaServerMessage::ModelPropsFetched(delivery) => {
                self.model_props.receive(delivery, clock);
                Task::none()
            }
            LlamaServerMessage::GgufCardRead(delivery) => {
                self.gguf_card.receive(delivery, clock);
                self.fetch_base_model_card(config, clock)
            }
            LlamaServerMessage::BaseModelCardFetched(delivery) => {
                self.base_model_card.receive(delivery, clock);
                Task::none()
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: LlamaServerInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<LlamaServerMessage> {
        match interaction {
            LlamaServerInteraction::User(scoped_interaction) => self
                .user
                .interact(scoped_interaction)
                .map(LlamaServerMessage::User),
            LlamaServerInteraction::System(scoped_interaction) => self
                .system
                .interact(scoped_interaction)
                .map(LlamaServerMessage::System),
            LlamaServerInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            LlamaServerInteraction::StartModelFetches(served_model) => {
                self.start_model_fetches(&served_model, config, clock)
            }
            LlamaServerInteraction::StopHoverFetches => {
                self.stop_hover_fetches();
                Task::none()
            }
        }
    }

    fn model_sections(&self) -> Vec<Section> {
        [
            self.model_props
                .sections_described_by(llama_model_props_sections),
            self.gguf_card
                .sections_described_by(gguf_model_card_sections),
            self.base_model_card
                .sections_described_by(hugging_face_model_card_sections),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::LlamaServer,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::LlamaServer.title(),
                Phosphor::Robot(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            LlamaServerInteraction::Expansion,
            IndicatorListing::Listed(
                self.user
                    .indicators(LlamaServerInteraction::User, move || self.model_sections())
                    .chain(
                        self.system
                            .indicators(LlamaServerInteraction::System, move || {
                                self.model_sections()
                            }),
                    )
                    .collect(),
            ),
        )
    }
}
