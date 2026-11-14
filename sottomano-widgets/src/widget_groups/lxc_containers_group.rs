use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sinks::lxc_containers::{start_lxc_container, stop_lxc_container};
use sottomano_desktop_sources::lxc_containers::{
    LxcContainer, LxcContainerListing, LxcContainerListings, LxcContainers, LxcProbe,
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
        process_usage_cells::{approximate_memory_text, running_display_state, running_state_text},
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
    container_name: Cow<'static, str>,
}

impl LxcContainerTarget {
    #[must_use]
    pub const fn of_container(container_name: Cow<'static, str>) -> Self {
        Self { container_name }
    }
}

impl From<LxcContainerTarget> for HoverTarget {
    fn from(target: LxcContainerTarget) -> Self {
        HoverTarget::LxcContainer(target)
    }
}

pub struct LxcContainerListingRow<'a> {
    listing: &'a LxcContainerListing,
}

impl GroupItemReading for LxcContainerListingRow<'_> {
    fn name(&self) -> &'static str {
        "LXC container"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.container_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
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

pub struct LxcContainerRow<'a> {
    container: &'a LxcContainer,
}

impl GroupItemReading for LxcContainerRow<'_> {
    fn name(&self) -> &'static str {
        "LXC container"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.container.container_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        if !self.container.running {
            return Some("stopped".to_owned());
        }
        self.container
            .memory_in_use
            .map(approximate_memory_text)
            .or_else(|| Some("running".to_owned()))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        running_display_state(self.container.running)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let container = self.container;
        let mut detail_entries = vec![LabeledValue::new(
            "State",
            running_state_text(container.running),
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
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Debug, Clone)]
pub struct LxcFamily;

pub type LxcContainersGroup = ContainerFamilyGroup<LxcFamily>;
pub type LxcContainersMessage = ContainerFamilyMessage<LxcFamily>;
pub type LxcContainersInteraction = ContainerFamilyInteraction<LxcFamily>;

impl ContainerFamily for LxcFamily {
    type Probe = LxcProbe;
    type Listings = LxcContainerListings;
    type Detail = LxcContainers;
    type Target = LxcContainerTarget;
    type PowerRequest = String;
    type ListingRow<'a> = LxcContainerListingRow<'a>;
    type DetailRow<'a> = LxcContainerRow<'a>;

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

    fn listing_rows(
        listings: &LxcContainerListings,
    ) -> Vec<FamilyRow<Self, LxcContainerListingRow<'_>>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: LxcContainerTarget::of_container(Cow::Owned(
                    listing.container_name.clone(),
                )),
                reading: LxcContainerListingRow { listing },
                power: if listing.running {
                    RowPower::Running(listing.container_name.clone())
                } else {
                    RowPower::Stopped(listing.container_name.clone())
                },
            })
            .collect()
    }

    fn detail_rows(detail: &LxcContainers) -> Vec<FamilyRow<Self, LxcContainerRow<'_>>> {
        detail
            .containers
            .iter()
            .map(|container| FamilyRow {
                target: LxcContainerTarget::of_container(Cow::Owned(
                    container.container_name.clone(),
                )),
                reading: LxcContainerRow { container },
                power: if container.running {
                    RowPower::Running(container.container_name.clone())
                } else {
                    RowPower::Stopped(container.container_name.clone())
                },
            })
            .collect()
    }

    fn group_label(probe: &LxcProbe) -> String {
        format!(
            "{} running, {} configured",
            probe.running_count, probe.configured_count
        )
    }

    fn group_hover_entries(probe: &LxcProbe) -> Vec<LabeledValue> {
        vec![
            LabeledValue::new("Running", probe.running_count.to_string()),
            LabeledValue::new("Configured", probe.configured_count.to_string()),
        ]
    }

    async fn stop(container_name: String) -> Result<(), String> {
        stop_lxc_container(&container_name).await
    }

    async fn start(container_name: String) -> Result<(), String> {
        start_lxc_container(&container_name).await
    }
}
