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
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::{HoverFetch, HoverFetchedSnapshot},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlamaServerReading {
    Switch,
    Model,
    GenerationRate,
    Slots,
    ContextFill,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlamaServerTarget {
    bus: Bus,
    unit: String,
    reading: LlamaServerReading,
}

impl From<LlamaServerTarget> for HoverTarget {
    fn from(target: LlamaServerTarget) -> Self {
        HoverTarget::LlamaServer(target)
    }
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

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        clock: Clock,
    ) -> Task<LlamaServerMessage> {
        self.model_props.stop();
        self.gguf_card.stop();
        self.base_model_card.stop();
        let Some(HoverTarget::LlamaServer(
            target @ LlamaServerTarget {
                reading: LlamaServerReading::Model,
                ..
            },
        )) = hovered
        else {
            return Task::none();
        };
        let servers = match target.bus {
            Bus::Session => &self.user,
            Bus::System => &self.system,
        };
        let props_fetch = servers.resident_instance(&target.unit).map(|instance| {
            self.model_props.start_hover_fetch(
                instance,
                LlamaServerMessage::ModelPropsFetched,
                config,
                clock,
            )
        });
        let card_read = servers.resident_model_file(&target.unit).map(|model_file| {
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
        if matches!(self.gguf_card.hover_fetch(), HoverFetch::Idle) {
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
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &LlamaServerTarget, clock: Clock) -> Option<HoverCard> {
        let model_groups: Vec<_> = [
            self.model_props.sections(clock),
            self.gguf_card.sections(clock),
            self.base_model_card.sections(clock),
        ]
        .into_iter()
        .flatten()
        .collect();
        match target.bus {
            Bus::Session => self.user.hover_card_for(target, model_groups, clock),
            Bus::System => self.system.hover_card_for(target, model_groups, clock),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, LlamaServerInteraction> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LlamaServer,
            &self.expansion,
            LlamaServerInteraction::Expansion,
            self.user
                .items(LlamaServerInteraction::User)
                .chain(self.system.items(LlamaServerInteraction::System)),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::Robot(Weight::Regular).into(),
        })
    }
}
