use std::collections::{BTreeMap, BTreeSet};

use super::{
    evdev_gamepad_devices::ConnectedGamepad, steam_dongle_hidraw::DongleInterface,
    steam_input_facts::PairingEvidence, steam_pairing_heuristics::sole_remaining_pair,
};
use crate::steam_client_files::{
    steam_controller_facts::SteamControllerFacts, virtual_gamepad_slots::SteamVirtualSlot,
};

const STEAM_PAD_NAME_MARKER: &str = "pad ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamPadAssignment<'facts> {
    pub interface: DongleInterface,
    pub virtual_slot: &'facts SteamVirtualSlot,
    pub evidence: PairingEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SteamControllerPad<'facts> {
    pad_slot: usize,
    virtual_slot: &'facts SteamVirtualSlot,
}

fn steam_pad_xinput_slot(pad_name: &str) -> Option<u8> {
    let (_, slot_digits) = pad_name.rsplit_once(STEAM_PAD_NAME_MARKER)?;
    slot_digits
        .parse()
        .inspect_err(|parse_failure| {
            tracing::debug!("gamepad: Steam pad {pad_name:?} has no slot number: {parse_failure}");
        })
        .ok()
}

fn steam_controller_pads<'facts>(
    roster: &[ConnectedGamepad],
    facts: &'facts SteamControllerFacts,
) -> Vec<SteamControllerPad<'facts>> {
    roster
        .iter()
        .filter(|gamepad| gamepad.is_steam_virtual_pad())
        .filter_map(|gamepad| {
            let xinput_slot = steam_pad_xinput_slot(&gamepad.name)?;
            let virtual_slot = facts
                .virtual_slots
                .iter()
                .find(|virtual_slot| virtual_slot.slot == xinput_slot)?;
            virtual_slot
                .is_steam_controller()
                .then_some(SteamControllerPad {
                    pad_slot: gamepad.slot,
                    virtual_slot,
                })
        })
        .collect()
}

struct AssignmentBook<'facts> {
    assignments: BTreeMap<usize, SteamPadAssignment<'facts>>,
}

impl<'facts> AssignmentBook<'facts> {
    fn interface_taken(&self, interface: &DongleInterface) -> bool {
        self.assignments
            .values()
            .any(|assignment| &assignment.interface == interface)
    }

    fn record(
        &mut self,
        pad: SteamControllerPad<'facts>,
        interface: &DongleInterface,
        evidence: PairingEvidence,
    ) {
        if self.assignments.contains_key(&pad.pad_slot) || self.interface_taken(interface) {
            return;
        }
        self.assignments.insert(
            pad.pad_slot,
            SteamPadAssignment {
                interface: interface.clone(),
                virtual_slot: pad.virtual_slot,
                evidence,
            },
        );
    }
}

#[must_use]
pub fn steam_pad_assignments<'facts>(
    roster: &[ConnectedGamepad],
    facts: &'facts SteamControllerFacts,
    known_interfaces: &BTreeMap<DongleInterface, bool>,
    correlated_interfaces: &BTreeMap<usize, DongleInterface>,
) -> BTreeMap<usize, SteamPadAssignment<'facts>> {
    let steam_pads = steam_controller_pads(roster, facts);
    let mut book = AssignmentBook {
        assignments: BTreeMap::new(),
    };
    for pad in &steam_pads {
        let logged_interface = known_interfaces.keys().find(|interface| {
            facts
                .controller_log
                .xinput_slot_behind(&interface.hidraw_node)
                == Some(pad.virtual_slot.slot)
        });
        if let Some(interface) = logged_interface {
            book.record(*pad, interface, PairingEvidence::SteamControllerLog);
        }
    }
    for pad in &steam_pads {
        if let Some(interface) = correlated_interfaces.get(&pad.pad_slot) {
            book.record(*pad, interface, PairingEvidence::InputTimingCorrelation);
        }
    }
    let unassigned_pads: Vec<SteamControllerPad<'facts>> = steam_pads
        .iter()
        .filter(|pad| !book.assignments.contains_key(&pad.pad_slot))
        .copied()
        .collect();
    let free_live_interfaces: Vec<&DongleInterface> = known_interfaces
        .iter()
        .filter(|(interface, live)| **live && !book.interface_taken(interface))
        .map(|(interface, _)| interface)
        .collect();
    if let Some((pad, interface)) = sole_remaining_pair(&unassigned_pads, &free_live_interfaces) {
        let interface = interface.clone();
        book.record(*pad, &interface, PairingEvidence::SoleRemainingCandidate);
    }
    book.assignments
}

#[must_use]
pub fn unassigned_steam_pad_slots(
    roster: &[ConnectedGamepad],
    facts: &SteamControllerFacts,
    assigned_pad_slots: &BTreeSet<usize>,
) -> BTreeSet<usize> {
    steam_controller_pads(roster, facts)
        .into_iter()
        .map(|pad| pad.pad_slot)
        .filter(|pad_slot| !assigned_pad_slots.contains(pad_slot))
        .collect()
}
