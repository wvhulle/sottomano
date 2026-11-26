use std::{borrow::Cow, collections::BTreeMap, mem::take, net::IpAddr};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    container_runtime_facts::{ContainerMemoryUse, ContainerStartTime},
    nspawn_containers::machined_nspawn_containers::{NspawnContainer, NspawnContainersReadout},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitActivity},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
        toggle_control::SwitchPresses,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage, activity_text},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[cfg(test)]
mod nspawn_container_tests;

const CONTAINER_SWITCH_FACE: UnitSwitchFace = UnitSwitchFace {
    name: Cow::Borrowed("Container"),
    icon: Phosphor::ShippingContainer(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the container",
        turning_off: "stop the container",
    },
};

fn preferred_address(ip_addresses: &[IpAddr]) -> Option<&IpAddr> {
    ip_addresses
        .iter()
        .find(|ip_address| ip_address.is_ipv4())
        .or_else(|| ip_addresses.first())
}

struct NspawnContainerIndicator<'view> {
    container: &'view NspawnContainer,
    shown_activity: UnitActivity,
}

impl NspawnContainerIndicator<'_> {
    fn detail_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let mut detail_entries = vec![LabeledValue::new(
            "State",
            activity_text(self.shown_activity),
        )];
        if !self.container.unit_name.is_empty() {
            detail_entries.push(LabeledValue::new("Unit", self.container.unit_name.clone()));
        }
        let Some(registered_machine) = self.container.registered_machine() else {
            return detail_entries;
        };
        detail_entries.extend(
            registered_machine
                .ip_addresses
                .iter()
                .map(|ip_address| LabeledValue::new("Address", ip_address.to_string())),
        );
        if let ContainerStartTime::StartedAt(started_at) = registered_machine.started_at {
            match clock.wall.duration_since(started_at) {
                Ok(uptime) => {
                    detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
                }
                Err(clock_skew) => tracing::debug!(
                    "nspawn containers: start time of {} lies ahead: {clock_skew}",
                    self.container.unit_name
                ),
            }
        }
        if let ContainerMemoryUse::Measured(memory_in_use) = registered_machine.memory_in_use {
            detail_entries.push(LabeledValue::new("Memory use", memory_in_use.to_string()));
        }
        detail_entries.push(LabeledValue::new(
            "Leader PID",
            registered_machine.leader_pid.to_string(),
        ));
        if !registered_machine.root_directory.as_os_str().is_empty() {
            detail_entries.push(LabeledValue::new(
                "Root",
                registered_machine.root_directory.display().to_string(),
            ));
        }
        detail_entries
    }
}

impl<'view> NspawnContainerIndicator<'view> {
    fn label(&self) -> Option<String> {
        match self.shown_activity {
            UnitActivity::Running => self
                .container
                .registered_machine()
                .and_then(|registered_machine| preferred_address(&registered_machine.ip_addresses))
                .map(ToString::to_string),
            activity => Some(activity_text(activity).to_owned()),
        }
    }

    fn state(&self) -> IndicatorState {
        match self.shown_activity {
            UnitActivity::Running => IndicatorState::On,
            UnitActivity::Starting | UnitActivity::Stopping => IndicatorState::Pending,
            UnitActivity::Stopped => IndicatorState::Off,
        }
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Container",
            dictionary_table(self.detail_entries(clock)),
        )]
    }

    fn indicator(self) -> GroupIndicator<'view> {
        let label = match self.label() {
            Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text),
            Some(_) | None => ReadoutLabel::Unlabeled,
        };
        let base = GroupIndicator::glyph_only(
            self.container.machine_name.clone(),
            Phosphor::ShippingContainer(Weight::Regular).into(),
            self.state(),
        )
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)));
        base.with_label(label)
    }
}

struct ContainerServiceSwitch {
    service_name: String,
    unit_switch: UnitSwitch,
}

impl ContainerServiceSwitch {
    fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_owned(),
            unit_switch: UnitSwitch::new(
                ScopedSystemdUnit {
                    scope: SystemdUnitScope::SystemWide,
                    unit_name: service_name.to_owned(),
                },
                CONTAINER_SWITCH_FACE,
            ),
        }
    }
}

#[derive(Default)]
pub struct NspawnContainersGroup {
    listing: SubscribedSnapshot<NspawnContainersReadout>,
    service_switches: BTreeMap<String, ContainerServiceSwitch>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum NspawnContainersMessage {
    Containers(SnapshotDelivery<NspawnContainersReadout>),
    Switch(String, UnitSwitchMessage),
}

#[derive(Debug, Clone)]
pub enum NspawnContainersInteraction {
    SwitchPressed(String),
    Expansion(IndicatorGroupExpansionMessage),
}

impl NspawnContainersGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<NspawnContainersMessage> {
        Subscription::batch(
            [
                snapshot_subscription::<NspawnContainersReadout>(buses.clone())
                    .map(NspawnContainersMessage::Containers),
            ]
            .into_iter()
            .chain(self.service_switches.iter().map(
                |(machine_name, service_switch)| {
                    service_switch
                        .unit_switch
                        .subscription(buses)
                        .with(machine_name.clone())
                        .map(|(machine_name, switch_message)| {
                            NspawnContainersMessage::Switch(machine_name, switch_message)
                        })
                },
            )),
        )
    }

    fn adopt_service_switches(&mut self) {
        let mut previous_switches = take(&mut self.service_switches);
        self.service_switches = self
            .listing
            .get()
            .into_iter()
            .flat_map(|readout| readout.containers.iter())
            .filter_map(|container| {
                let service_name = container.controllable_service()?;
                let service_switch = previous_switches
                    .remove(&container.machine_name)
                    .filter(|kept_switch| kept_switch.service_name == service_name)
                    .unwrap_or_else(|| ContainerServiceSwitch::new(service_name));
                Some((container.machine_name.clone(), service_switch))
            })
            .collect();
    }

    pub fn update(
        &mut self,
        message: NspawnContainersMessage,
        clock: Clock,
    ) -> Task<NspawnContainersMessage> {
        match message {
            NspawnContainersMessage::Containers(delivery) => {
                self.listing.receive(delivery, clock);
                self.adopt_service_switches();
                Task::none()
            }
            NspawnContainersMessage::Switch(machine_name, switch_message) => {
                match self.service_switches.get_mut(&machine_name) {
                    Some(service_switch) => service_switch
                        .unit_switch
                        .update(switch_message, clock)
                        .map(move |switch_message| {
                            NspawnContainersMessage::Switch(machine_name.clone(), switch_message)
                        }),
                    None => Task::none(),
                }
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: NspawnContainersInteraction,
    ) -> Task<NspawnContainersMessage> {
        match interaction {
            NspawnContainersInteraction::SwitchPressed(machine_name) => {
                match self.service_switches.get_mut(&machine_name) {
                    Some(service_switch) => {
                        service_switch
                            .unit_switch
                            .press()
                            .map(move |switch_message| {
                                NspawnContainersMessage::Switch(
                                    machine_name.clone(),
                                    switch_message,
                                )
                            })
                    }
                    None => Task::none(),
                }
            }
            NspawnContainersInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn container_indicator<'view>(
        &'view self,
        container: &'view NspawnContainer,
    ) -> NspawnContainerIndicator<'view> {
        NspawnContainerIndicator {
            container,
            shown_activity: self
                .service_switches
                .get(&container.machine_name)
                .and_then(|service_switch| service_switch.unit_switch.activity())
                .unwrap_or(container.activity),
        }
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let containers = self
            .listing
            .get()
            .map(|readout| readout.containers.as_slice())
            .unwrap_or_default();
        let rows = containers.iter().map(|container| {
            let row = self
                .container_indicator(container)
                .indicator()
                .with_hover_card(Box::new(move |clock| {
                    self.listing
                        .hover_card_with(self.container_indicator(container).hover_card(clock))
                }));
            match self.service_switches.get(&container.machine_name) {
                Some(service_switch) => service_switch.unit_switch.attach_switch_press(
                    row,
                    NspawnContainersInteraction::SwitchPressed(container.machine_name.clone()),
                ),
                None => row,
            }
        });
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::NspawnContainers,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::NspawnContainers.title(),
                Phosphor::Hexagon(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            NspawnContainersInteraction::Expansion,
            IndicatorListing::Listed(rows.collect()),
        )
    }
}
