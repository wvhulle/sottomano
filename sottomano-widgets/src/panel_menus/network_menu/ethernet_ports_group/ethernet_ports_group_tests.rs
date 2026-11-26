use std::time::Duration;

use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::{
        ethernet_connection::{Ethernet, PortState},
        interface_bus_attachment::InterfaceAttachment,
        network_manager_proxies::LinkPhase,
    },
};

use super::{EthernetPortsGroup, EthernetPortsMessage};
use crate::clock::Clock;

fn port_attached(port_name: &str, attachment: InterfaceAttachment) -> PortState {
    PortState {
        name: port_name.to_owned(),
        attachment,
        link_state: LinkPhase::Down,
    }
}

#[test]
fn every_port_gets_a_group_with_its_own_title() {
    let clock = Clock::real();
    let mut ethernet_group = EthernetPortsGroup::default();
    let _ = ethernet_group.update(
        EthernetPortsMessage::Snapshot(SnapshotDelivery::fixture_delivered(
            Ok(Ethernet {
                ports: vec![
                    port_attached("enp2s0", InterfaceAttachment::BuiltIn),
                    port_attached("enp16s0f4u2", InterfaceAttachment::Usb),
                    port_attached("enp16s0f4u3", InterfaceAttachment::Usb),
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(60))),
        )),
        clock,
    );
    let group_titles: Vec<String> = ethernet_group
        .group_contents()
        .iter()
        .map(|group| group.group_title.clone().to_string())
        .collect();
    assert_eq!(group_titles, ["Ethernet", "USB Ethernet", "USB Ethernet 2"]);
}
