use std::{
    borrow::Cow,
    future::{Future, ready},
};

use chrono::Locale;
use sottomano_desktop_sinks::apptainer_containers::stop_apptainer_container;
use sottomano_desktop_sources::{
    apptainer_containers::{ApptainerContainer, ApptainerContainerListing, ApptainerProbe},
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
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
pub struct ApptainerContainerTarget {
    pub runtime_pid: i32,
}

impl From<ApptainerContainerTarget> for HoverTarget {
    fn from(target: ApptainerContainerTarget) -> Self {
        HoverTarget::ApptainerContainer(target)
    }
}

pub struct ApptainerContainerListingRow<'a> {
    listing: &'a ApptainerContainerListing,
}

impl GroupItemReading for ApptainerContainerListingRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        format!(
            "{} pid {}",
            self.listing.container_name, self.listing.runtime_pid
        )
        .into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(ContainerRunState::Running.label().to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ContainerRunState::Running.into()
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([LabeledValue::new(
                "State",
                ContainerRunState::Running.label(),
            )]),
        )]
    }
}

pub struct ApptainerContainerRow<'a> {
    container: &'a ApptainerContainer,
}

impl GroupItemReading for ApptainerContainerRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        format!(
            "{} pid {}",
            self.container.listing.container_name, self.container.listing.runtime_pid
        )
        .into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(approximate_memory_text(self.container.memory_in_use))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let container = self.container;
        let mut detail_entries = Vec::new();
        if let Some(image_file) = &container.image_file {
            detail_entries.push(LabeledValue::new("Image", image_file.display().to_string()));
        }
        if let Some(running_command) = &container.running_command {
            detail_entries.push(LabeledValue::new("Running", running_command.clone()));
        }
        detail_entries.push(LabeledValue::new(
            "Memory use",
            approximate_memory_text(container.memory_in_use),
        ));
        detail_entries.extend(uptime_entry(clock, container.started_at));
        detail_entries.extend([
            LabeledValue::new(
                "Owner",
                container
                    .owner_name
                    .clone()
                    .unwrap_or_else(|| container.owner_uid.to_string()),
            ),
            LabeledValue::new("Runtime", container.runtime_title.clone()),
            LabeledValue::new("PID", container.listing.runtime_pid.to_string()),
        ]);
        vec![Section::new("Instance", dictionary_table(detail_entries))]
    }
}

#[derive(Debug, Clone)]
pub struct ApptainerFamily;

pub type ApptainerContainersGroup = ContainerFamilyGroup<ApptainerFamily>;
pub type ApptainerContainersMessage = ContainerFamilyMessage<ApptainerFamily>;
pub type ApptainerContainersInteraction = ContainerFamilyInteraction<ApptainerFamily>;

impl ContainerFamily for ApptainerFamily {
    type Probe = ApptainerProbe;
    type Listings = FamilyListings<ApptainerContainerListing>;
    type Detail = FamilyDetails<ApptainerContainer>;
    type Target = ApptainerContainerTarget;
    type PowerRequest = i32;
    type ListingRow<'a> = ApptainerContainerListingRow<'a>;
    type DetailRow<'a> = ApptainerContainerRow<'a>;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::ApptainerContainers;
    const STOP_HINT: &'static str = "stop the container";
    const GROUP_NAME: &'static str = "Apptainer containers";

    fn group_glyph() -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn running_count(probe: &ApptainerProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(
        listings: &FamilyListings<ApptainerContainerListing>,
    ) -> Vec<FamilyRow<Self, ApptainerContainerListingRow<'_>>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: ApptainerContainerTarget {
                    runtime_pid: listing.runtime_pid,
                },
                reading: ApptainerContainerListingRow { listing },
                power: RowPower::Stoppable(listing.runtime_pid),
            })
            .collect()
    }

    fn detail_rows(
        detail: &FamilyDetails<ApptainerContainer>,
    ) -> Vec<FamilyRow<Self, ApptainerContainerRow<'_>>> {
        detail
            .details
            .iter()
            .map(|container| FamilyRow {
                target: ApptainerContainerTarget {
                    runtime_pid: container.listing.runtime_pid,
                },
                reading: ApptainerContainerRow { container },
                power: RowPower::Stoppable(container.listing.runtime_pid),
            })
            .collect()
    }

    fn stop(runtime_pid: i32) -> impl Future<Output = Result<(), String>> + Send + 'static {
        ready(stop_apptainer_container(runtime_pid))
    }
}
