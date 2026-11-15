use crate::group_items::reading_display_state::ReadingDisplayState;
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
    ai_usage_spend::{gguf_header_model_card::GgufModelCard, llama_server_slots::LlamaModelProps},
    system_protocol_clients::dbus_desktop_buses::{Bus, DesktopBuses},
    systemd_units_journal::SystemdUnitScope,
    voice_dictation_input::huggingface_model_card::HuggingFaceModelCard,
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{toggle_control::SwitchPresses, unit_switch::UnitSwitchFace},
    hover_cards::hover_card_body::Section,
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::{HoverFetchedSnapshot, SnapshotFetchStage},
        phosphor::{Phosphor, Weight},
    },
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
    widget_groups::llama_server_group::{
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
    expansion: WidgetGroupExpansion,
}

impl Default for LlamaServerGroup {
    fn default() -> Self {
        Self {
            user: ScopedLlamaServers::new(SystemdUnitScope::UserSession),
            system: ScopedLlamaServers::new(SystemdUnitScope::SystemWide),
            model_props: HoverFetchedSnapshot::default(),
            gguf_card: HoverFetchedSnapshot::default(),
            base_model_card: HoverFetchedSnapshot::default(),
            expansion: WidgetGroupExpansion::default(),
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
    Interaction(LlamaServerInteraction),
}

#[derive(Debug, Clone)]
pub enum LlamaServerInteraction {
    User(ScopedServersInteraction),
    System(ScopedServersInteraction),
    Expansion(WidgetGroupExpansionMessage),
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
        let resident_instance = servers.resident_instance(&served_model.unit);
        let resident_model_file = servers.resident_model_file(&served_model.unit);
        let props_fetch = resident_instance.map(|instance| {
            self.model_props.start_hover_fetch(
                instance,
                LlamaServerMessage::ModelPropsFetched,
                config,
                clock,
            )
        });
        let card_read = resident_model_file.map(|model_file| {
            self.gguf_card.start_hover_fetch(
                model_file,
                LlamaServerMessage::GgufCardRead,
                config,
                clock,
            )
        });
        Task::batch(props_fetch.into_iter().chain(card_read))
    }

    fn fetch_base_model_card(&mut self, config: &Config, clock: Clock) -> Task<LlamaServerMessage> {
        if matches!(self.gguf_card.fetch_stage(), SnapshotFetchStage::Idle) {
            return Task::none();
        }
        let Some(repository) = self
            .gguf_card
            .reading()
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
            LlamaServerMessage::Interaction(LlamaServerInteraction::User(interaction)) => self
                .user
                .interact(interaction)
                .map(LlamaServerMessage::User),
            LlamaServerMessage::Interaction(LlamaServerInteraction::System(interaction)) => self
                .system
                .interact(interaction)
                .map(LlamaServerMessage::System),
            LlamaServerMessage::Interaction(LlamaServerInteraction::Expansion(message)) => {
                self.expansion.update(message);
                Task::none()
            }
            LlamaServerMessage::Interaction(LlamaServerInteraction::StartModelFetches(
                served_model,
            )) => self.start_model_fetches(&served_model, config, clock),
            LlamaServerMessage::Interaction(LlamaServerInteraction::StopHoverFetches) => {
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

    pub fn group_content(&self) -> WidgetGroupContent<'_, LlamaServerInteraction> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LlamaServer,
            &self.expansion,
            LlamaServerInteraction::Expansion,
            self.user
                .items(LlamaServerInteraction::User, move || self.model_sections())
                .chain(self.system.items(LlamaServerInteraction::System, move || {
                    self.model_sections()
                })),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::Robot(Weight::Regular).into(),
            state: ReadingDisplayState::Idle,
        })
    }
}
