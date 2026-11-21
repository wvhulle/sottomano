use sottomano_desktop_sinks::{
    lxc_containers::change_lxc_container, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::{ContainerRunState, RunChange},
    container_runtime_facts::ContainerMemoryUse,
    lxc_containers::{LxcConfigFile, LxcContainer, LxcContainerListing, LxcProbe},
};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        container_family_group::{
            ContainerFamily, ContainerFamilyGroup, ContainerFamilyInteraction,
            ContainerFamilyMessage, FamilyIndicator, RowPower,
        },
        indicator_group_identity::IndicatorGroupFamily,
        process_usage_cells::approximate_memory_text,
    },
    widgets::value_cells::LabeledValue,
};

pub struct LxcContainerListingIndicator<'view> {
    listing: &'view LxcContainerListing,
}

impl<'view> LxcContainerListingIndicator<'view> {
    fn indicator(self) -> GroupIndicator<'view> {
        let listing = self.listing;
        GroupIndicator::glyph_only(
            listing.container_name.clone(),
            Phosphor::Package(Weight::Regular).into(),
            listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(listing.run_state.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| {
            vec![Section::new(
                "Status",
                dictionary_table([LabeledValue::new("State", listing.run_state.label())]),
            )]
        }))
    }
}

pub struct LxcContainerIndicator<'view> {
    container: &'view LxcContainer,
}

impl<'view> LxcContainerIndicator<'view> {
    fn indicator(self) -> GroupIndicator<'view> {
        let container = self.container;
        let label = match container.listing.run_state {
            ContainerRunState::Stopped => ContainerRunState::Stopped.label().to_owned(),
            ContainerRunState::Running => match container.memory_in_use {
                ContainerMemoryUse::Measured(memory_in_use) => {
                    approximate_memory_text(memory_in_use)
                }
                ContainerMemoryUse::Unaccounted => ContainerRunState::Running.label().to_owned(),
            },
        };
        GroupIndicator::glyph_only(
            container.listing.container_name.clone(),
            Phosphor::Package(Weight::Regular).into(),
            container.listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| Self::hover_card(container)))
    }

    fn hover_card(container: &LxcContainer) -> Vec<Section> {
        let mut detail_entries = vec![LabeledValue::new(
            "State",
            container.listing.run_state.label(),
        )];
        if let ContainerMemoryUse::Measured(memory_in_use) = container.memory_in_use {
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
        if let LxcConfigFile::InLxcPath(config_path) = &container.config_path {
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
    type PowerRequest = String;

    const FAMILY: IndicatorGroupFamily = IndicatorGroupFamily::LxcContainers;
    const STOP_HINT: &'static str = "stop the container";
    const START_HINT: &'static str = "start the container";
    const GROUP_NAME: &'static str = "LXC containers";

    fn group_glyph() -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn running_count(probe: &LxcProbe) -> usize {
        probe.running_count
    }

    fn listing_indicators(
        listings: &FamilyListings<LxcContainerListing>,
    ) -> Vec<FamilyIndicator<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyIndicator {
                indicator: LxcContainerListingIndicator { listing }.indicator(),
                power: container_power(listing),
            })
            .collect()
    }

    fn detail_indicators(detail: &FamilyDetails<LxcContainer>) -> Vec<FamilyIndicator<'_, Self>> {
        detail
            .details
            .iter()
            .map(|container| FamilyIndicator {
                indicator: LxcContainerIndicator { container }.indicator(),
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
