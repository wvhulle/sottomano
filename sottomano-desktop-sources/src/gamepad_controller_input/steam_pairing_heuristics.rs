use std::{collections::BTreeMap, time::Duration};

use tokio::time::Instant;

use super::steam_dongle_hidraw::DongleInterface;

pub const CORRELATION_WINDOW: Duration = Duration::from_millis(25);

#[must_use]
pub fn serial_number_from_steam_handle(steam_handle: u64) -> String {
    let handle_digits = format!("{steam_handle:X}");
    let (family_letter, unit_digits) = handle_digits.split_at(handle_digits.len().min(1));
    format!("{family_letter}X{unit_digits}")
}

#[must_use]
pub fn sole_remaining_pair<'pads, 'interfaces, Pad>(
    unassigned_pads: &'pads [Pad],
    free_live_interfaces: &'interfaces [&'interfaces DongleInterface],
) -> Option<(&'pads Pad, &'interfaces DongleInterface)> {
    let [pad] = unassigned_pads else {
        return None;
    };
    let [interface] = free_live_interfaces else {
        return None;
    };
    Some((pad, interface))
}

#[must_use]
pub fn interface_changing_with_edge(
    edge_at: Instant,
    latest_button_change: &BTreeMap<DongleInterface, Instant>,
    is_candidate: impl Fn(&DongleInterface) -> bool,
) -> Option<&DongleInterface> {
    let mut candidates = latest_button_change
        .iter()
        .filter(|(interface, changed_at)| {
            is_candidate(interface)
                && edge_at.saturating_duration_since(**changed_at) <= CORRELATION_WINDOW
                && changed_at.saturating_duration_since(edge_at) <= CORRELATION_WINDOW
        })
        .map(|(interface, _)| interface);
    let sole_candidate = candidates.next()?;
    candidates.next().is_none().then_some(sole_candidate)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn dongle_slot(interface_number: u8) -> DongleInterface {
        DongleInterface {
            interface_number,
            dongle_serial: "FXB0000000009".to_owned(),
            kernel_device: format!("+hid:{interface_number}"),
            hidraw_node: PathBuf::from(format!("/dev/hidraw1{interface_number}")),
        }
    }

    #[test]
    fn the_handle_spells_the_serial_without_its_x() {
        assert_eq!(serial_number_from_steam_handle(0xfa00_0000_0001), "FXA0000000001");
    }

    #[test]
    fn only_a_single_pad_and_a_single_slot_pair_by_elimination() {
        let second_slot = dongle_slot(2);
        let third_slot = dongle_slot(3);
        assert!(sole_remaining_pair(&[18], &[&second_slot]).is_some());
        assert!(sole_remaining_pair(&[18], &[&second_slot, &third_slot]).is_none());
        assert!(sole_remaining_pair::<usize>(&[], &[&second_slot]).is_none());
    }

    #[test]
    fn two_slots_changing_together_are_ambiguous() {
        let edge_at = Instant::now();
        let changes = BTreeMap::from([(dongle_slot(2), edge_at), (dongle_slot(3), edge_at)]);
        assert_eq!(interface_changing_with_edge(edge_at, &changes, |_| true), None);
        assert_eq!(
            interface_changing_with_edge(edge_at, &changes, |interface| {
                interface.interface_number == 3
            }),
            Some(&dongle_slot(3))
        );
    }
}
