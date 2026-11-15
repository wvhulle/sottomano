use std::path::PathBuf;

use chrono::Locale;
use sottomano_desktop_sinks::{
    sink_write_errors::GuestPowerError, vmware_machines::stop_vmware_machine,
};
use sottomano_desktop_sources::{
    container_family_snapshots::{FamilyDetails, FamilyListings},
    vmware_machines::{VmwareMachine, VmwareMachineListing, VmwareMachineProbe},
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
    },
    widget_groups::{
        container_family_group::{
            ContainerFamily, ContainerFamilyGroup, ContainerFamilyInteraction,
            ContainerFamilyMessage, FamilyRow, RowPower,
        },
        process_usage_cells::{approximate_memory_text, uptime_entry},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmwareMachineTarget {
    pub pid: i32,
}

impl From<VmwareMachineTarget> for HoverTarget {
    fn from(target: VmwareMachineTarget) -> Self {
        HoverTarget::VmwareMachine(target)
    }
}

pub struct VmwareMachineListingRow<'a> {
    listing: &'a VmwareMachineListing,
}

impl<'a> VmwareMachineListingRow<'a> {
    #[must_use]
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled("running".to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.listing.machine_name.clone(),
                Phosphor::Desktop(Weight::Regular).into(),
                ReadingDisplayState::On,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("State", "running"),
                LabeledValue::new("PID", self.listing.pid.to_string()),
            ]),
        )]
    }
}

pub struct VmwareMachineRow<'a> {
    machine: &'a VmwareMachine,
}

impl<'a> VmwareMachineRow<'a> {
    #[must_use]
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.machine.listing.machine_name.clone(),
                Phosphor::Desktop(Weight::Regular).into(),
                ReadingDisplayState::On,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let machine = self.machine;
        let mut detail_entries = vec![LabeledValue::new(
            "Memory use",
            approximate_memory_text(machine.memory_in_use),
        )];
        if let Some(guest_os) = &machine.guest_os {
            detail_entries.push(LabeledValue::new("Guest", guest_os.clone()));
        }
        if let Some(guest_memory) = machine.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let Some(guest_cores) = machine.guest_cores {
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
    type Target = VmwareMachineTarget;
    type PowerRequest = PathBuf;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::VmwareMachines;
    const STOP_HINT: &'static str = "shut the machine down";
    const GROUP_NAME: &'static str = "VMware machines";

    fn group_glyph() -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn running_count(probe: &VmwareMachineProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(
        listings: &FamilyListings<VmwareMachineListing>,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: VmwareMachineTarget { pid: listing.pid },
                reading: VmwareMachineListingRow { listing }.reading_row(clock, locale),
                power: RowPower::Stoppable(listing.configuration_file.clone()),
            })
            .collect()
    }

    fn detail_rows(
        detail: &FamilyDetails<VmwareMachine>,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        detail
            .details
            .iter()
            .map(|machine| FamilyRow {
                target: VmwareMachineTarget {
                    pid: machine.listing.pid,
                },
                reading: VmwareMachineRow { machine }.reading_row(clock, locale),
                power: RowPower::Stoppable(machine.listing.configuration_file.clone()),
            })
            .collect()
    }

    async fn stop(configuration_file: PathBuf) -> Result<(), GuestPowerError> {
        stop_vmware_machine(&configuration_file).await
    }
}
