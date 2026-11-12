use std::{borrow::Cow, collections::BTreeMap, mem::take, net::IpAddr};

use chrono::Locale;
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
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::SwitchPresses,
        unit_switch::{UnitSwitch, UnitSwitchFace, UnitSwitchMessage},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const CONTAINER_SWITCH_FACE: UnitSwitchFace = UnitSwitchFace {
    name: "Container",
    icon: Phosphor::ShippingContainer(Weight::Regular),
    presses: SwitchPresses {
        turning_on: "start the container",
        turning_off: "stop the container",
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NspawnContainerTarget {
    machine_name: Cow<'static, str>,
}

impl NspawnContainerTarget {
    #[must_use]
    pub const fn of_machine(machine_name: Cow<'static, str>) -> Self {
        Self { machine_name }
    }
}

impl From<NspawnContainerTarget> for HoverTarget {
    fn from(target: NspawnContainerTarget) -> Self {
        HoverTarget::NspawnContainer(target)
    }
}

fn activity_text(activity: UnitActivity) -> &'static str {
    match activity {
        UnitActivity::Running => "running",
        UnitActivity::Starting => "starting",
        UnitActivity::Stopping => "stopping",
        UnitActivity::Stopped => "stopped",
    }
}

fn preferred_address(ip_addresses: &[IpAddr]) -> Option<&IpAddr> {
    ip_addresses
        .iter()
        .find(|ip_address| ip_address.is_ipv4())
        .or_else(|| ip_addresses.first())
}

struct NspawnContainerRow<'a> {
    container: &'a NspawnContainer,
    switched_activity: Option<UnitActivity>,
}

impl NspawnContainerRow<'_> {
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

impl GroupItemReading for NspawnContainerRow<'_> {
    fn name(&self) -> &'static str {
        "Container"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.container.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ShippingContainer(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.shown_activity() {
            UnitActivity::Running => self
                .container
                .registered_machine()
                .and_then(|registered_machine| preferred_address(&registered_machine.ip_addresses))
                .map(ToString::to_string),
            activity => Some(activity_text(activity).to_owned()),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.shown_activity() {
            UnitActivity::Running => ReadingDisplayState::On,
            UnitActivity::Starting | UnitActivity::Stopping => ReadingDisplayState::Pending,
            UnitActivity::Stopped => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.detail_entries(clock))
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
    listing: LatestSnapshot<NspawnContainersReadout, HidesFetchFailures>,
    service_switches: BTreeMap<String, ContainerServiceSwitch>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum NspawnContainersMessage {
    Containers(SnapshotDelivery<NspawnContainersReadout>),
    Switch(String, UnitSwitchMessage),
    Interaction(NspawnContainersInteraction),
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
            NspawnContainersMessage::Interaction(NspawnContainersInteraction::SwitchPressed(
                machine_name,
            )) => match self.service_switches.get_mut(&machine_name) {
                Some(service_switch) => {
                    service_switch
                        .unit_switch
                        .press()
                        .map(move |switch_message| {
                            NspawnContainersMessage::Switch(machine_name.clone(), switch_message)
                        })
                }
                None => Task::none(),
            },
            NspawnContainersMessage::Interaction(NspawnContainersInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn container_row<'a>(&'a self, container: &'a NspawnContainer) -> NspawnContainerRow<'a> {
        NspawnContainerRow {
            container,
            switched_activity: self
                .service_switches
                .get(&container.machine_name)
                .and_then(|service_switch| service_switch.unit_switch.activity()),
        }
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &NspawnContainerTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        let container = self
            .listing
            .get()?
            .containers
            .iter()
            .find(|container| container.machine_name == target.machine_name)?;
        self.listing
            .hover_card_with(self.container_row(container).hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, NspawnContainersInteraction> {
        let containers = self
            .listing
            .get()
            .map(|readout| readout.containers.as_slice())
            .unwrap_or_default();
        let rows = containers.iter().map(|container| {
            let row = GroupItem::reading(
                self.container_row(container),
                NspawnContainerTarget::of_machine(Cow::Owned(container.machine_name.clone())),
            );
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
            &self.expansion,
            NspawnContainersInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ShippingContainer(Weight::Regular).into(),
        })
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
        let clock = clock_at_wall(SystemTime::UNIX_EPOCH + Duration::from_hours(2));
        let container = running_container(SystemTime::UNIX_EPOCH);
        let row = NspawnContainerRow {
            container: &container,
            switched_activity: None,
        };
        assert_eq!(
            row.label(clock, Locale::en_US),
            Some("10.200.0.2".to_owned())
        );
    }

    #[test]
    fn a_pending_switch_overrides_the_listed_activity() {
        let clock = clock_at_wall(SystemTime::UNIX_EPOCH);
        let container = running_container(SystemTime::UNIX_EPOCH);
        let row = NspawnContainerRow {
            container: &container,
            switched_activity: Some(UnitActivity::Stopping),
        };
        assert_eq!(row.label(clock, Locale::en_US), Some("stopping".to_owned()));
        assert_eq!(row.state(clock), ReadingDisplayState::Pending);
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
        let row = NspawnContainerRow {
            container: &container,
            switched_activity: None,
        };
        assert_eq!(row.detail_entries(clock).len(), 2);
        assert_eq!(row.state(clock), ReadingDisplayState::Off);
    }

    #[test]
    fn uptime_is_listed_from_the_machine_start_time() {
        let started_at = SystemTime::UNIX_EPOCH;
        let clock = clock_at_wall(started_at + Duration::from_mins(90));
        let container = running_container(started_at);
        let row = NspawnContainerRow {
            container: &container,
            switched_activity: None,
        };
        assert!(row.detail_entries(clock).contains(&LabeledValue::new(
            "Uptime",
            ValueCell::duration(Duration::from_mins(90))
        )));
    }
}
