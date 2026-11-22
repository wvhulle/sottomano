use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    lxc_containers::LxcContainerListing,
    virtualbox_machines::{
        RegisteredVirtualBoxMachine, VirtualBoxFrontend, VirtualBoxMachine,
        VirtualBoxMachineListing, VirtualBoxMachineListings, VirtualBoxSettingsFile,
    },
    vmware_machines::{VmwareGuestOs, VmwareMachine, VmwareMachineListing},
};

use super::{ContainerFamily, ContainerFamilyGroup, ContainerFamilyInteraction, RowPower};
use crate::{
    clock::Clock,
    hover_cards::{HoverTarget, IndicatorKey, hover_card_body::Section},
    indicator_groups::{
        indicator_group_identity::IndicatorGroupIdentity,
        indicator_group_interaction::IndicatorGroupInteraction,
        lxc_containers_group::LxcFamily,
        virtualbox_machines_group::{VirtualBoxMachinesGroup, VirtualBoxMachinesMessage},
        vmware_machines_group::{VmwareMachinesGroup, VmwareMachinesMessage},
    },
};

const LISTED_PID: i32 = 11;
const DETAILED_PID: i32 = 22;

fn polled<Source: SnapshotSource>(snapshot: Source) -> SnapshotDelivery<Source> {
    SnapshotDelivery::fixture_delivered(
        Ok(snapshot),
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
    )
}

fn shown_card_sections<Family: ContainerFamily>(
    group: &ContainerFamilyGroup<Family>,
    row_name: &'static str,
    clock: Clock,
) -> Vec<Section>
where
    ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
{
    let hover_target = HoverTarget::Indicator(IndicatorKey {
        group: IndicatorGroupIdentity::sole(Family::FAMILY),
        name: row_name.into(),
    });
    group
        .group_content()
        .hover_card(&hover_target, clock)
        .map(|indicator_card| indicator_card.sections)
        .unwrap_or_default()
}

fn group_with_listing(clock: Clock) -> VmwareMachinesGroup {
    let mut group = VmwareMachinesGroup::default();
    let _ = group.update(
        VmwareMachinesMessage::Listings(polled(FamilyListings {
            listings: vec![VmwareMachineListing {
                pid: LISTED_PID,
                machine_name: "listed".to_owned(),
                configuration_file: PathBuf::from("/vm/listed.vmx"),
            }],
        })),
        clock,
    );
    group
}

fn detailed_machines(clock: Clock) -> FamilyDetails<VmwareMachine> {
    FamilyDetails {
        details: vec![VmwareMachine {
            listing: VmwareMachineListing {
                pid: DETAILED_PID,
                machine_name: "detailed".to_owned(),
                configuration_file: PathBuf::from("/vm/detailed.vmx"),
            },
            guest_os: VmwareGuestOs::Undeclared,
            guest_memory: GuestMemorySize::HypervisorDefault,
            guest_cores: GuestCoreCount::HypervisorDefault,
            started_at: clock.wall,
            memory_in_use: ByteSize::mib(512),
        }],
    }
}

#[test]
fn listings_answer_until_detail_arrives() {
    let clock = Clock::real();
    let group = group_with_listing(clock);
    assert_eq!(group.group_rows().len(), 1);
    assert!(!shown_card_sections(&group, "listed", clock).is_empty());
}

#[test]
fn detail_replaces_listings_once_it_has_indicators() {
    let clock = Clock::real();
    let mut group = group_with_listing(clock);
    let _ = group.update(
        VmwareMachinesMessage::Detail(polled(detailed_machines(clock))),
        clock,
    );
    assert_eq!(group.group_rows().len(), 1);
    assert!(!shown_card_sections(&group, "detailed", clock).is_empty());
    assert!(shown_card_sections(&group, "listed", clock).is_empty());
}

#[test]
fn running_containers_switch_off_and_stopped_ones_switch_on() {
    let listings = FamilyListings {
        listings: vec![
            LxcContainerListing {
                container_name: "running".to_owned(),
                run_state: ContainerRunState::Running,
            },
            LxcContainerListing {
                container_name: "stopped".to_owned(),
                run_state: ContainerRunState::Stopped,
            },
        ],
    };
    let presses: Vec<String> = LxcFamily::listing_indicators(&listings)
        .into_iter()
        .map(|row| match row.power {
            RowPower::SwitchedOn(container_name) => format!("switch off {container_name}"),
            RowPower::SwitchedOff(container_name) => format!("switch on {container_name}"),
            RowPower::Stoppable(container_name) => format!("stop {container_name}"),
        })
        .collect();
    assert_eq!(presses, vec!["switch off running", "switch on stopped"]);
}

#[test]
fn stopped_machines_follow_the_detailed_running_ones() {
    let clock = Clock::real();
    let running_listing = VirtualBoxMachineListing {
        pid: DETAILED_PID,
        machine_name: "windows11".to_owned(),
        machine_reference: "windows11".to_owned(),
    };
    let mut group = VirtualBoxMachinesGroup::default();
    let _ = group.update(
        VirtualBoxMachinesMessage::Listings(polled(VirtualBoxMachineListings {
            listings: vec![running_listing.clone()],
            stopped_machines: vec![RegisteredVirtualBoxMachine {
                machine_name: "ubuntu".to_owned(),
                machine_uuid: "0b1c2d3e".to_owned(),
            }],
        })),
        clock,
    );
    let _ = group.update(
        VirtualBoxMachinesMessage::Detail(polled(FamilyDetails {
            details: vec![VirtualBoxMachine {
                listing: running_listing,
                frontend: VirtualBoxFrontend::Headless,
                guest_memory: GuestMemorySize::HypervisorDefault,
                guest_cores: GuestCoreCount::HypervisorDefault,
                settings_file: VirtualBoxSettingsFile::NotInDefaultFolder,
                started_at: clock.wall,
                memory_in_use: ByteSize::gib(2),
            }],
        })),
        clock,
    );
    assert_eq!(group.group_rows().len(), 2);
    assert!(!shown_card_sections(&group, "ubuntu 0b1c2d3e", clock).is_empty());
}
