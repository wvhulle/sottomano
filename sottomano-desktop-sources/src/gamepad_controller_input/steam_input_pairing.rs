use std::collections::{BTreeMap, BTreeSet};

use tokio::time::Instant;

use super::{
    evdev_gamepad_devices::{
        ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower, GamepadTransport,
        InputRoute, PhysicalGamepadLink, RawGamepadEvent,
    },
    player_indicator_leds::PlayerIndicator,
    steam_dongle_hidraw::{DongleInterface, DongleSample},
    steam_input_assignment::{
        SteamPadAssignment, steam_pad_assignments, unassigned_steam_pad_slots,
    },
    steam_input_facts::SteamInputFacts,
    steam_pairing_heuristics::{interface_changing_with_edge, serial_number_from_steam_handle},
};
use crate::steam_client_files::steam_controller_facts::SteamControllerFacts;

pub const STEAM_CONTROLLER_NAME: &str = "Steam Controller";

#[derive(Debug)]
pub enum DeviceSignal {
    Raw(RawGamepadEvent),
    Dongle(DongleInterface, DongleSample),
    DongleButtonsChanged(DongleInterface, Instant),
    SteamFacts(Box<SteamControllerFacts>),
}

#[derive(Debug, Default)]
pub struct SteamInputPairing {
    latest_roster: Option<Vec<ConnectedGamepad>>,
    delivered_roster: Vec<ConnectedGamepad>,
    known_interfaces: BTreeMap<DongleInterface, bool>,
    battery_by_interface: BTreeMap<DongleInterface, GamepadPower>,
    steam_facts: SteamControllerFacts,
    correlated_interfaces: BTreeMap<usize, DongleInterface>,
    latest_button_change: BTreeMap<DongleInterface, Instant>,
}

fn steam_input_controller(
    steam_virtual_pad: &ConnectedGamepad,
    assignment: &SteamPadAssignment<'_>,
    power: GamepadPower,
    facts: &SteamControllerFacts,
) -> ConnectedGamepad {
    let controller_serial = serial_number_from_steam_handle(assignment.virtual_slot.steam_handle);
    let input_facts = SteamInputFacts::gathered(
        facts,
        assignment.evidence,
        &controller_serial,
        &assignment.interface.dongle_serial,
    );
    ConnectedGamepad {
        slot: steam_virtual_pad.slot,
        name: STEAM_CONTROLLER_NAME.to_owned(),
        vendor_id: steam_virtual_pad.vendor_id,
        product_id: steam_virtual_pad.product_id,
        power,
        origin: GamepadOrigin::Physical(PhysicalGamepadLink {
            kernel_device: assignment.interface.kernel_device.clone(),
            transport: GamepadTransport::SteamDongle,
            identity: ControllerIdentity::Addressed {
                hardware_address: controller_serial,
                alias: STEAM_CONTROLLER_NAME.to_owned(),
            },
            player_indicator: PlayerIndicator::Unlit,
            input_route: InputRoute::SteamInput(input_facts),
        }),
    }
}

impl SteamInputPairing {
    fn assignments<'pairing>(
        &'pairing self,
        roster: &[ConnectedGamepad],
    ) -> BTreeMap<usize, SteamPadAssignment<'pairing>> {
        steam_pad_assignments(
            roster,
            &self.steam_facts,
            &self.known_interfaces,
            &self.correlated_interfaces,
        )
    }

    fn paired_roster(&self, roster: &[ConnectedGamepad]) -> Vec<ConnectedGamepad> {
        let assignments = self.assignments(roster);
        roster
            .iter()
            .map(|gamepad| {
                let Some(assignment) = assignments.get(&gamepad.slot) else {
                    return gamepad.clone();
                };
                let power = self
                    .battery_by_interface
                    .get(&assignment.interface)
                    .copied()
                    .unwrap_or(GamepadPower::NoBattery);
                steam_input_controller(gamepad, assignment, power, &self.steam_facts)
            })
            .collect()
    }

    fn deliver_roster(&mut self) -> Option<RawGamepadEvent> {
        let paired = self.paired_roster(self.latest_roster.as_deref()?);
        if paired == self.delivered_roster {
            return None;
        }
        self.delivered_roster.clone_from(&paired);
        Some(RawGamepadEvent::Roster(paired))
    }

    fn correlate_button_edge(&mut self, pad_slot: usize, edge_at: Instant) -> bool {
        let Some(roster) = self.latest_roster.as_deref() else {
            return false;
        };
        let assigned_pad_slots: BTreeSet<usize> = self.assignments(roster).into_keys().collect();
        if !unassigned_steam_pad_slots(roster, &self.steam_facts, &assigned_pad_slots)
            .contains(&pad_slot)
        {
            return false;
        }
        let assigned_interfaces: BTreeSet<DongleInterface> = self
            .assignments(roster)
            .into_values()
            .map(|assignment| assignment.interface)
            .collect();
        let Some(interface) =
            interface_changing_with_edge(edge_at, &self.latest_button_change, |interface| {
                !assigned_interfaces.contains(interface)
            })
        else {
            return false;
        };
        self.correlated_interfaces
            .insert(pad_slot, interface.clone());
        true
    }

    pub fn absorb(&mut self, signal: DeviceSignal) -> Vec<RawGamepadEvent> {
        match signal {
            DeviceSignal::Raw(RawGamepadEvent::Roster(roster)) => {
                self.correlated_interfaces
                    .retain(|pad_slot, _| roster.iter().any(|gamepad| gamepad.slot == *pad_slot));
                let paired = self.paired_roster(&roster);
                self.latest_roster = Some(roster);
                self.delivered_roster.clone_from(&paired);
                vec![RawGamepadEvent::Roster(paired)]
            }
            DeviceSignal::Raw(button @ RawGamepadEvent::Button { slot, .. }) => {
                let newly_paired = self.correlate_button_edge(slot, Instant::now());
                let roster_update = newly_paired.then(|| self.deliver_roster()).flatten();
                roster_update.into_iter().chain([button]).collect()
            }
            DeviceSignal::Raw(event) => vec![event],
            DeviceSignal::Dongle(interface, sample) => {
                if let DongleSample::Battery(power) = sample {
                    self.battery_by_interface.insert(interface.clone(), power);
                }
                self.known_interfaces
                    .insert(interface, sample != DongleSample::Silent);
                self.deliver_roster().into_iter().collect()
            }
            DeviceSignal::DongleButtonsChanged(interface, changed_at) => {
                self.latest_button_change.insert(interface, changed_at);
                Vec::new()
            }
            DeviceSignal::SteamFacts(facts) => {
                self.steam_facts = *facts;
                self.deliver_roster().into_iter().collect()
            }
        }
    }
}

#[cfg(test)]
mod steam_input_pairing_tests;
