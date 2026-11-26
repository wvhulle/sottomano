use std::path::PathBuf;

use sottomano_desktop_sinks::{
    sink_write_errors::GuestPowerError, vmware_machines::stop_vmware_machine,
};
use sottomano_desktop_sources::{
    container_family_snapshots::{FamilyDetails, FamilyListings},
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    vmware_machines::{VmwareGuestOs, VmwareMachine, VmwareMachineListing, VmwareMachineProbe},
};

use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        process_usage_cells::{approximate_memory_text, uptime_entry},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::virtualization_menu::container_family_group::{
        ContainerFamily, ContainerFamilyGroup, ContainerFamilyInteraction, ContainerFamilyMessage,
        FamilyIndicator, RowPower,
    },
    widgets::value_cells::LabeledValue,
};

pub struct VmwareMachineListingIndicator<'view> {
    listing: &'view VmwareMachineListing,
}

impl<'view> VmwareMachineListingIndicator<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.listing.machine_name.clone(),
            Phosphor::Desktop(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("running".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    #[must_use]
    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", "running"),
                LabeledValue::new("PID", self.listing.pid.to_string()),
            ]),
        )]
    }
}

pub struct VmwareMachineIndicator<'view> {
    machine: &'view VmwareMachine,
}

impl<'view> VmwareMachineIndicator<'view> {
    #[must_use]
    pub fn indicator(self) -> GroupIndicator<'view> {
        GroupIndicator::glyph_only(
            self.machine.listing.machine_name.clone(),
            Phosphor::Desktop(Weight::Regular).into(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let machine = self.machine;
        let mut detail_entries = vec![LabeledValue::new(
            "Memory use",
            approximate_memory_text(machine.memory_in_use),
        )];
        if let VmwareGuestOs::Declared(guest_os) = &machine.guest_os {
            detail_entries.push(LabeledValue::new("Guest", guest_os.clone()));
        }
        if let GuestMemorySize::Configured(guest_memory) = machine.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let GuestCoreCount::Configured(guest_cores) = machine.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        detail_entries.extend(uptime_entry(clock, machine.started_at));
        detail_entries.extend([
            LabeledValue::new(
                "Configuration",
                machine.listing.configuration_file.display().to_string(),
            ),
            LabeledValue::new("PID", machine.listing.pid.to_string()),
        ]);
        vec![Section::new("Machine", dictionary_table(detail_entries))]
    }
}

#[derive(Debug, Clone)]
pub struct VmwareFamily;

pub type VmwareMachinesGroup = ContainerFamilyGroup<VmwareFamily>;
pub type VmwareMachinesMessage = ContainerFamilyMessage<VmwareFamily>;
pub type VmwareMachinesInteraction = ContainerFamilyInteraction<VmwareFamily>;

impl ContainerFamily for VmwareFamily {
    type Probe = VmwareMachineProbe;
    type Listings = FamilyListings<VmwareMachineListing>;
    type Detail = FamilyDetails<VmwareMachine>;
    type PowerRequest = PathBuf;

    const FAMILY: IndicatorGroupFamily = IndicatorGroupFamily::VmwareMachines;
    const STOP_HINT: &'static str = "shut the machine down";
    const GROUP_NAME: &'static str = "VMware machines";

    fn group_glyph() -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn running_count(probe: &VmwareMachineProbe) -> usize {
        probe.running_count
    }

    fn listing_indicators(
        listings: &FamilyListings<VmwareMachineListing>,
    ) -> Vec<FamilyIndicator<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyIndicator {
                indicator: VmwareMachineListingIndicator { listing }.indicator(),
                power: RowPower::Stoppable(listing.configuration_file.clone()),
            })
            .collect()
    }

    fn detail_indicators(detail: &FamilyDetails<VmwareMachine>) -> Vec<FamilyIndicator<'_, Self>> {
        detail
            .details
            .iter()
            .map(|machine| FamilyIndicator {
                indicator: VmwareMachineIndicator { machine }.indicator(),
                power: RowPower::Stoppable(machine.listing.configuration_file.clone()),
            })
            .collect()
    }

    async fn stop(configuration_file: PathBuf) -> Result<(), GuestPowerError> {
        stop_vmware_machine(&configuration_file).await
    }
}
