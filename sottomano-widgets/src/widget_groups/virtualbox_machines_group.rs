use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sinks::{
    sink_write_errors::GuestPowerError,
    virtualbox_machines::{press_virtualbox_power_button, start_virtualbox_machine},
};
use sottomano_desktop_sources::{
    container_family_snapshots::FamilyDetails,
    container_run_state::ContainerRunState,
    virtualbox_machines::{
        RegisteredVirtualBoxMachine, VirtualBoxFrontend, VirtualBoxMachine,
        VirtualBoxMachineListing, VirtualBoxMachineListings, VirtualBoxMachineProbe,
    },
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
    widgets::{icon_glyph::IconGlyph, simple_icons::SimpleIcon, value_cells::LabeledValue},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualBoxMachineTarget {
    RunningProcess { pid: i32 },
    StoppedMachine { machine_uuid: Cow<'static, str> },
}

impl From<VirtualBoxMachineTarget> for HoverTarget {
    fn from(target: VirtualBoxMachineTarget) -> Self {
        HoverTarget::VirtualBoxMachine(target)
    }
}

pub enum VirtualBoxListedRow<'a> {
    Running(&'a VirtualBoxMachineListing),
    Stopped(&'a RegisteredVirtualBoxMachine),
}

impl VirtualBoxListedRow<'_> {
    const fn run_state(&self) -> ContainerRunState {
        match self {
            VirtualBoxListedRow::Running(_) => ContainerRunState::Running,
            VirtualBoxListedRow::Stopped(_) => ContainerRunState::Stopped,
        }
    }
}

impl<'a> VirtualBoxListedRow<'a> {
    #[must_use]
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        let name = match &self {
            VirtualBoxListedRow::Running(listing) => {
                format!("{} pid {}", listing.machine_name, listing.pid)
            }
            VirtualBoxListedRow::Stopped(machine) => {
                format!("{} {}", machine.machine_name, machine.machine_uuid)
            }
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(self.run_state().label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                name,
                IconGlyph::SimpleIcon(SimpleIcon::VirtualBox),
                self.run_state().into(),
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let state_entry = LabeledValue::new("State", self.run_state().label());
        let identity_entries = match self {
            VirtualBoxListedRow::Running(listing) => vec![
                LabeledValue::new("Machine", listing.machine_reference.clone()),
                LabeledValue::new("PID", listing.pid.to_string()),
            ],
            VirtualBoxListedRow::Stopped(machine) => {
                vec![LabeledValue::new("UUID", machine.machine_uuid.clone())]
            }
        };
        vec![Section::new(
            "Status",
            dictionary_table([state_entry].into_iter().chain(identity_entries)),
        )]
    }
}

pub struct VirtualBoxMachineRow<'a> {
    machine: &'a VirtualBoxMachine,
}

impl<'a> VirtualBoxMachineRow<'a> {
    #[must_use]
    pub fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        let name = format!(
            "{} pid {}",
            self.machine.listing.machine_name, self.machine.listing.pid
        );
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(approximate_memory_text(self.machine.memory_in_use)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                name,
                IconGlyph::SimpleIcon(SimpleIcon::VirtualBox),
                ReadingDisplayState::On,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    #[must_use]
    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let machine = self.machine;
        let mut detail_entries = vec![
            LabeledValue::new(
                "Window",
                match machine.frontend {
                    VirtualBoxFrontend::Headless => "headless",
                    VirtualBoxFrontend::Windowed => "own window",
                },
            ),
            LabeledValue::new("Memory use", approximate_memory_text(machine.memory_in_use)),
        ];
        if let Some(guest_memory) = machine.guest_memory {
            detail_entries.push(LabeledValue::new("Guest memory", guest_memory.to_string()));
        }
        if let Some(guest_cores) = machine.guest_cores {
            detail_entries.push(LabeledValue::new("Cores", guest_cores.to_string()));
        }
        detail_entries.extend(uptime_entry(clock, machine.started_at));
        if let Some(settings_file) = &machine.settings_file {
            detail_entries.push(LabeledValue::new(
                "Settings",
                settings_file.display().to_string(),
            ));
        }
        detail_entries.extend([
            LabeledValue::new("Machine", machine.listing.machine_reference.clone()),
            LabeledValue::new("PID", machine.listing.pid.to_string()),
        ]);
        vec![Section::new("Machine", dictionary_table(detail_entries))]
    }
}

#[derive(Debug, Clone)]
pub struct VirtualBoxFamily;

pub type VirtualBoxMachinesGroup = ContainerFamilyGroup<VirtualBoxFamily>;
pub type VirtualBoxMachinesMessage = ContainerFamilyMessage<VirtualBoxFamily>;
pub type VirtualBoxMachinesInteraction = ContainerFamilyInteraction<VirtualBoxFamily>;

impl ContainerFamily for VirtualBoxFamily {
    type Probe = VirtualBoxMachineProbe;
    type Listings = VirtualBoxMachineListings;
    type Detail = FamilyDetails<VirtualBoxMachine>;
    type Target = VirtualBoxMachineTarget;
    type PowerRequest = String;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::VirtualBoxMachines;
    const STOP_HINT: &'static str = "press the guest's power button";
    const START_HINT: &'static str = "start the machine";
    const GROUP_NAME: &'static str = "VirtualBox machines";

    fn group_glyph() -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::VirtualBox)
    }

    fn running_count(probe: &VirtualBoxMachineProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(
        listings: &VirtualBoxMachineListings,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: VirtualBoxMachineTarget::RunningProcess { pid: listing.pid },
                reading: VirtualBoxListedRow::Running(listing).reading_row(clock, locale),
                power: RowPower::SwitchedOn(listing.machine_reference.clone()),
            })
            .collect()
    }

    fn detail_rows(
        detail: &FamilyDetails<VirtualBoxMachine>,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        detail
            .details
            .iter()
            .map(|machine| FamilyRow {
                target: VirtualBoxMachineTarget::RunningProcess {
                    pid: machine.listing.pid,
                },
                reading: VirtualBoxMachineRow { machine }.reading_row(clock, locale),
                power: RowPower::SwitchedOn(machine.listing.machine_reference.clone()),
            })
            .collect()
    }

    fn stopped_rows(
        listings: &VirtualBoxMachineListings,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        listings
            .stopped_machines
            .iter()
            .map(|machine| FamilyRow {
                target: VirtualBoxMachineTarget::StoppedMachine {
                    machine_uuid: Cow::Owned(machine.machine_uuid.clone()),
                },
                reading: VirtualBoxListedRow::Stopped(machine).reading_row(clock, locale),
                power: RowPower::SwitchedOff(machine.machine_uuid.clone()),
            })
            .collect()
    }

    async fn stop(machine_reference: String) -> Result<(), GuestPowerError> {
        press_virtualbox_power_button(&machine_reference).await
    }

    async fn start(machine_uuid: String) -> Result<(), GuestPowerError> {
        start_virtualbox_machine(&machine_uuid).await
    }
}
