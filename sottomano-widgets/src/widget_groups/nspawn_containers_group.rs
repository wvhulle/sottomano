use std::{borrow::Cow, collections::BTreeMap, mem::take, net::IpAddr};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    nspawn_containers::machined_nspawn_containers::{NspawnContainer, NspawnContainersReadout},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::{ScopedSystemdUnit, SystemdUnitScope, unit_active_state::UnitActivity},
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
        toggle_control::SwitchPresses,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage, activity_text},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::{LabeledValue, ValueCell},
    },
};

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

struct NspawnContainerIndicator<'a> {
    container: &'a NspawnContainer,
    switched_activity: Option<UnitActivity>,
}

impl NspawnContainerIndicator<'_> {
    fn shown_activity(&self) -> UnitActivity {
        self.switched_activity.unwrap_or(self.container.activity)
    }

    fn detail_entries(&self, clock: Clock) -> Vec<LabeledValue> {
        let mut detail_entries = vec![LabeledValue::new(
            "State",
            activity_text(self.shown_activity()),
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
        if let Some(uptime) = registered_machine.started_at.and_then(|started_at| {
            clock
                .wall
                .duration_since(started_at)
                .inspect_err(|clock_skew| {
                    tracing::debug!(
                        "nspawn containers: start time of {} lies ahead: {clock_skew}",
                        self.container.unit_name
                    );
                })
                .ok()
        }) {
            detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
        }
        if let Some(memory_in_use) = registered_machine.memory_in_use {
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

impl<'a> NspawnContainerIndicator<'a> {
    fn label(&self) -> Option<String> {
        match self.shown_activity() {
            UnitActivity::Running => self
                .container
                .registered_machine()
                .and_then(|registered_machine| preferred_address(&registered_machine.ip_addresses))
                .map(ToString::to_string),
            activity => Some(activity_text(activity).to_owned()),
        }
    }

    fn state(&self) -> IndicatorState {
        match self.shown_activity() {
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

    fn indicator(self) -> GroupIndicator<'a> {
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
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum NspawnContainersMessage {
    Containers(SnapshotDelivery<NspawnContainersReadout>),
    Switch(String, UnitSwitchMessage),
}

#[derive(Debug, Clone)]
pub enum NspawnContainersInteraction {
    SwitchPressed(String),
    Expansion(WidgetGroupExpansionMessage),
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

    fn container_indicator<'a>(
        &'a self,
        container: &'a NspawnContainer,
    ) -> NspawnContainerIndicator<'a> {
        NspawnContainerIndicator {
            container,
            switched_activity: self
                .service_switches
                .get(&container.machine_name)
                .and_then(|service_switch| service_switch.unit_switch.activity()),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_> {
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
        WidgetGroupContent::expandable(
            WidgetGroupFamily::NspawnContainers,
            GroupIndicator::glyph_only(
                WidgetGroupFamily::NspawnContainers.title(),
                Phosphor::Hexagon(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.expansion,
            NspawnContainersInteraction::Expansion,
            rows,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{Ipv4Addr, Ipv6Addr},
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use sottomano_desktop_sources::nspawn_containers::machined_nspawn_containers::{
        ContainerRegistration, RegisteredContainerMachine,
    };

    use super::*;

    fn clock_at_wall(wall: SystemTime) -> Clock {
        Clock {
            wall,
            ..Clock::real()
        }
    }

    fn running_container(started_at: SystemTime) -> NspawnContainer {
        NspawnContainer {
            machine_name: "arr-proton".to_owned(),
            unit_name: "container@arr-proton.service".to_owned(),
            activity: UnitActivity::Running,
            registration: ContainerRegistration::Registered(RegisteredContainerMachine {
                leader_pid: 3_110_493,
                started_at: Some(started_at),
                ip_addresses: vec![
                    IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 2)),
                    IpAddr::V4(Ipv4Addr::new(10, 200, 0, 2)),
                ],
                root_directory: PathBuf::from("/var/lib/nixos-containers/arr-proton"),
                memory_in_use: None,
            }),
        }
    }

    #[test]
    fn a_running_container_is_labelled_with_its_ipv4_address() {
        let container = running_container(SystemTime::UNIX_EPOCH);
        let row = NspawnContainerIndicator {
            container: &container,
            switched_activity: None,
        };
        assert_eq!(row.label(), Some("10.200.0.2".to_owned()));
    }

    #[test]
    fn a_pending_switch_overrides_the_listed_activity() {
        let container = running_container(SystemTime::UNIX_EPOCH);
        let row = NspawnContainerIndicator {
            container: &container,
            switched_activity: Some(UnitActivity::Stopping),
        };
        assert_eq!(row.label(), Some("stopping".to_owned()));
        assert_eq!(row.state(), IndicatorState::Pending);
    }

    #[test]
    fn a_stopped_container_lists_only_its_state_and_unit() {
        let clock = clock_at_wall(SystemTime::UNIX_EPOCH);
        let container = NspawnContainer {
            machine_name: "backup".to_owned(),
            unit_name: "container@backup.service".to_owned(),
            activity: UnitActivity::Stopped,
            registration: ContainerRegistration::Unregistered,
        };
        let row = NspawnContainerIndicator {
            container: &container,
            switched_activity: None,
        };
        assert_eq!(row.detail_entries(clock).len(), 2);
        assert_eq!(row.state(), IndicatorState::Off);
    }

    #[test]
    fn uptime_is_listed_from_the_machine_start_time() {
        let started_at = SystemTime::UNIX_EPOCH;
        let clock = clock_at_wall(started_at + Duration::from_mins(90));
        let container = running_container(started_at);
        let row = NspawnContainerIndicator {
            container: &container,
            switched_activity: None,
        };
        assert!(row.detail_entries(clock).contains(&LabeledValue::new(
            "Uptime",
            ValueCell::duration(Duration::from_mins(90))
        )));
    }
}
