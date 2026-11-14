use std::{borrow::Cow, path::PathBuf};

use chrono::Locale;
use sottomano_desktop_sinks::vmware_machines::stop_vmware_machine;
use sottomano_desktop_sources::vmware_machines::{
    VmwareMachine, VmwareMachineListing, VmwareMachineListings, VmwareMachineProbe, VmwareMachines,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::{HoverTarget, hover_card_body::HoverCardBody},
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
    pid: i32,
}

impl VmwareMachineTarget {
    #[must_use]
    pub const fn of_pid(pid: i32) -> Self {
        Self { pid }
    }
}

impl From<VmwareMachineTarget> for HoverTarget {
    fn from(target: VmwareMachineTarget) -> Self {
        HoverTarget::VmwareMachine(target)
    }
}

pub struct VmwareMachineListingRow<'a> {
    listing: &'a VmwareMachineListing,
}

impl GroupItemReading for VmwareMachineListingRow<'_> {
    fn name(&self) -> &'static str {
        "VMware machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("running".to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("State", "running"),
            LabeledValue::new("PID", self.listing.pid.to_string()),
        ])
    }
}

pub struct VmwareMachineRow<'a> {
    machine: &'a VmwareMachine,
}

impl GroupItemReading for VmwareMachineRow<'_> {
    fn name(&self) -> &'static str {
        "VMware machine"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.machine.machine_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(approximate_memory_text(self.machine.memory_in_use))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
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
                machine.configuration_file.display().to_string(),
            ),
            LabeledValue::new("PID", machine.pid.to_string()),
        ]);
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Debug, Clone)]
pub struct VmwareFamily;

pub type VmwareMachinesGroup = ContainerFamilyGroup<VmwareFamily>;
pub type VmwareMachinesMessage = ContainerFamilyMessage<VmwareFamily>;
pub type VmwareMachinesInteraction = ContainerFamilyInteraction<VmwareFamily>;

impl ContainerFamily for VmwareFamily {
    type Probe = VmwareMachineProbe;
    type Listings = VmwareMachineListings;
    type Detail = VmwareMachines;
    type Target = VmwareMachineTarget;
    type PowerRequest = PathBuf;
    type ListingRow<'a> = VmwareMachineListingRow<'a>;
    type DetailRow<'a> = VmwareMachineRow<'a>;

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
        listings: &VmwareMachineListings,
    ) -> Vec<FamilyRow<Self, VmwareMachineListingRow<'_>>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: VmwareMachineTarget::of_pid(listing.pid),
                reading: VmwareMachineListingRow { listing },
                power: RowPower::Running(listing.configuration_file.clone()),
            })
            .collect()
    }

    fn detail_rows(detail: &VmwareMachines) -> Vec<FamilyRow<Self, VmwareMachineRow<'_>>> {
        detail
            .machines
            .iter()
            .map(|machine| FamilyRow {
                target: VmwareMachineTarget::of_pid(machine.pid),
                reading: VmwareMachineRow { machine },
                power: RowPower::Running(machine.configuration_file.clone()),
            })
            .collect()
    }

    async fn stop(configuration_file: PathBuf) -> Result<(), String> {
        stop_vmware_machine(&configuration_file).await
    }
}
