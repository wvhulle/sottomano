use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::apptainer_containers::{
    ApptainerContainer, ApptainerContainerListing, ApptainerContainerListings, ApptainerContainers,
    ApptainerProbe,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::{HoverTarget, hover_card_body::HoverCardBody},
    widget_groups::{
        container_family_group::{
            ContainerFamily, ContainerFamilyGroup, ContainerFamilyMessage, FamilyRow,
        },
        process_usage_cells::{
            approximate_memory_text, running_display_state, running_state_text, uptime_entry,
        },
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
    runtime_pid: i32,
}

impl ApptainerContainerTarget {
    #[must_use]
    pub const fn of_runtime(runtime_pid: i32) -> Self {
        Self { runtime_pid }
    }
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
    fn name(&self) -> &'static str {
        "Apptainer container"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.container_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(running_state_text(self.listing.running).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        running_display_state(self.listing.running)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
            "State",
            running_state_text(self.listing.running),
        )])
    }
}

pub struct ApptainerContainerRow<'a> {
    container: &'a ApptainerContainer,
}

impl GroupItemReading for ApptainerContainerRow<'_> {
    fn name(&self) -> &'static str {
        "Apptainer container"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.container.shown_name().into()
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

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
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
            LabeledValue::new("PID", container.runtime_pid.to_string()),
        ]);
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Debug)]
pub struct ApptainerFamily;

pub type ApptainerContainersGroup = ContainerFamilyGroup<ApptainerFamily>;
pub type ApptainerContainersMessage = ContainerFamilyMessage<ApptainerFamily>;

impl ContainerFamily for ApptainerFamily {
    type Probe = ApptainerProbe;
    type Listings = ApptainerContainerListings;
    type Detail = ApptainerContainers;
    type Target = ApptainerContainerTarget;
    type ListingRow<'a> = ApptainerContainerListingRow<'a>;
    type DetailRow<'a> = ApptainerContainerRow<'a>;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::ApptainerContainers;
    const GROUP_NAME: &'static str = "Apptainer containers";

    fn group_glyph() -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn running_count(probe: &ApptainerProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(
        listings: &ApptainerContainerListings,
    ) -> Vec<FamilyRow<ApptainerContainerTarget, ApptainerContainerListingRow<'_>>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: ApptainerContainerTarget::of_runtime(listing.runtime_pid),
                reading: ApptainerContainerListingRow { listing },
            })
            .collect()
    }

    fn detail_rows(
        detail: &ApptainerContainers,
    ) -> Vec<FamilyRow<ApptainerContainerTarget, ApptainerContainerRow<'_>>> {
        detail
            .containers
            .iter()
            .map(|container| FamilyRow {
                target: ApptainerContainerTarget::of_runtime(container.runtime_pid),
                reading: ApptainerContainerRow { container },
            })
            .collect()
    }
}
