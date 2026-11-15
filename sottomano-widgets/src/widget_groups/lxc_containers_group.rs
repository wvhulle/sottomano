use std::borrow::Cow;

use sottomano_desktop_sinks::{
    lxc_containers::change_lxc_container, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::{ContainerRunState, RunChange},
    lxc_containers::{LxcContainer, LxcContainerListing, LxcProbe},
};

use crate::{
    group_items::{
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
        process_usage_cells::approximate_memory_text,
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcContainerTarget {
    pub container_name: Cow<'static, str>,
}

impl From<LxcContainerTarget> for HoverTarget {
    fn from(target: LxcContainerTarget) -> Self {
        HoverTarget::LxcContainer(target)
    }
}

pub struct LxcContainerListingRow<'a> {
    listing: &'a LxcContainerListing,
}

impl<'a> LxcContainerListingRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let listing = self.listing;
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(listing.run_state.label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                listing.container_name.clone(),
                Phosphor::Package(Weight::Regular).into(),
                listing.run_state.into(),
                Box::new(move |_| {
                    vec![Section::new(
                        "Status",
                        dictionary_table([LabeledValue::new("State", listing.run_state.label())]),
                    )]
                }),
            )
        }
    }
}

pub struct LxcContainerRow<'a> {
    container: &'a LxcContainer,
}

impl<'a> LxcContainerRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let container = self.container;
        let label = match container.listing.run_state {
            ContainerRunState::Stopped => ContainerRunState::Stopped.label().to_owned(),
            ContainerRunState::Running => container.memory_in_use.map_or_else(
                || ContainerRunState::Running.label().to_owned(),
                approximate_memory_text,
            ),
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                container.listing.container_name.clone(),
                Phosphor::Package(Weight::Regular).into(),
                container.listing.run_state.into(),
                Box::new(move |_| Self::hover_card(container)),
            )
        }
    }

    fn hover_card(container: &LxcContainer) -> Vec<Section> {
        let mut detail_entries = vec![LabeledValue::new(
            "State",
            container.listing.run_state.label(),
        )];
        if let Some(memory_in_use) = container.memory_in_use {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(memory_in_use),
            ));
        }
        detail_entries.extend(
            container
                .configured_addresses
                .iter()
                .map(|configured_address| LabeledValue::new("Address", configured_address.clone())),
        );
        if let Some(config_path) = &container.config_path {
            detail_entries.push(LabeledValue::new(
                "Config",
                config_path.display().to_string(),
            ));
        }
        vec![Section::new("Container", dictionary_table(detail_entries))]
    }
}

#[derive(Debug, Clone)]
pub struct LxcFamily;

pub type LxcContainersGroup = ContainerFamilyGroup<LxcFamily>;
pub type LxcContainersMessage = ContainerFamilyMessage<LxcFamily>;
pub type LxcContainersInteraction = ContainerFamilyInteraction<LxcFamily>;

impl ContainerFamily for LxcFamily {
    type Probe = LxcProbe;
    type Listings = FamilyListings<LxcContainerListing>;
    type Detail = FamilyDetails<LxcContainer>;
    type Target = LxcContainerTarget;
    type PowerRequest = String;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::LxcContainers;
    const STOP_HINT: &'static str = "stop the container";
    const START_HINT: &'static str = "start the container";
    const GROUP_NAME: &'static str = "LXC containers";

    fn group_glyph() -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn running_count(probe: &LxcProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(listings: &FamilyListings<LxcContainerListing>) -> Vec<FamilyRow<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: LxcContainerTarget {
                    container_name: Cow::Owned(listing.container_name.clone()),
                },
                reading: LxcContainerListingRow { listing }.reading_row(),
                power: container_power(listing),
            })
            .collect()
    }

    fn detail_rows(detail: &FamilyDetails<LxcContainer>) -> Vec<FamilyRow<'_, Self>> {
        detail
            .details
            .iter()
            .map(|container| FamilyRow {
                target: LxcContainerTarget {
                    container_name: Cow::Owned(container.listing.container_name.clone()),
                },
                reading: LxcContainerRow { container }.reading_row(),
                power: container_power(&container.listing),
            })
            .collect()
    }

    async fn stop(container_name: String) -> Result<(), GuestPowerError> {
        change_lxc_container(&container_name, RunChange::Stop).await
    }

    async fn start(container_name: String) -> Result<(), GuestPowerError> {
        change_lxc_container(&container_name, RunChange::Start).await
    }
}

fn container_power(listing: &LxcContainerListing) -> RowPower<String> {
    match listing.run_state {
        ContainerRunState::Running => RowPower::SwitchedOn(listing.container_name.clone()),
        ContainerRunState::Stopped => RowPower::SwitchedOff(listing.container_name.clone()),
    }
}
