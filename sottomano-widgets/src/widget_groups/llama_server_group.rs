mod context_fill_gauge;
mod generation_rate_label;
mod loaded_model_label;
mod scoped_servers;
mod slot_occupancy_gauge;

use iced::{Subscription, Task};
use scoped_servers::{ScopedLlamaServers, ScopedServersMessage};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    ai_usage_spend::llama_server_slots::LlamaModelProps,
    system_protocol_clients::dbus_desktop_buses::{Bus, DesktopBuses},
    systemd_units_journal::SystemdUnitScope,
};

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        toggle_control::SwitchPresses,
        tooltip_body::Tooltip,
        unit_switch::UnitSwitchFace,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

const SERVER_SWITCH: UnitSwitchFace = UnitSwitchFace {
    name: "llama-server",
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
    model_props: OnHoverReading<LlamaModelProps>,
    expansion: GroupExpansion,
}

impl Default for LlamaServerGroup {
    fn default() -> Self {
        Self {
            user: ScopedLlamaServers::new(SystemdUnitScope::UserSession),
            system: ScopedLlamaServers::new(SystemdUnitScope::SystemWide),
            model_props: OnHoverReading::default(),
            expansion: GroupExpansion::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LlamaServerMessage {
    User(ScopedServersMessage),
    System(ScopedServersMessage),
    ModelPropsFetched(SourceDelivery<LlamaModelProps>),
    Expansion(ExpansionMessage),
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
        let Some(HoverTarget::LlamaServer(
            target @ LlamaServerTarget {
                reading: LlamaServerReading::Model,
                ..
            },
        )) = hovered
        else {
            return Task::none();
        };
        let instance = match target.bus {
            Bus::Session => self.user.resident_instance(&target.unit),
            Bus::System => self.system.resident_instance(&target.unit),
        };
        let Some(instance) = instance else {
            return Task::none();
        };
        self.model_props.start_hover_fetch(
            instance,
            LlamaServerMessage::ModelPropsFetched,
            config,
            clock,
        )
    }

    pub fn update(
        &mut self,
        message: LlamaServerMessage,
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
                self.model_props.receive_delivery(delivery, clock);
                Task::none()
            }
            LlamaServerMessage::Expansion(message) => {
                self.expansion.update(message);
                Task::none()
            }
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &LlamaServerTarget, clock: Clock) -> Option<Tooltip> {
        let model_props = self.model_props.sections(clock);
        match target.bus {
            Bus::Session => self.user.tooltip_for(target, model_props, clock),
            Bus::System => self.system.tooltip_for(target, model_props, clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, LlamaServerMessage> {
        let resident = self
            .user
            .resident_models()
            .chain(self.system.resident_models())
            .next();
        GroupContent::new(
            &self.expansion,
            LlamaServerMessage::Expansion,
            self.user
                .items(LlamaServerMessage::User)
                .chain(self.system.items(LlamaServerMessage::System)),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Robot(Weight::Regular).into(),
            handle: GroupHandle::LlamaServer,
        })
        .preview(resident.map(|model| PreviewedValue::Plain(model)))
    }
}
