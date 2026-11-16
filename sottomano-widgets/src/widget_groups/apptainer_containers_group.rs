use std::future::{Future, ready};

use sottomano_desktop_sinks::{
    apptainer_containers::stop_apptainer_container, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    apptainer_containers::{ApptainerContainer, ApptainerContainerListing, ApptainerProbe},
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
};

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
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

pub struct ApptainerContainerListingRow<'a> {
    listing: &'a ApptainerContainerListing,
}

impl<'a> ApptainerContainerListingRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let name = format!(
            "{} pid {}",
            self.listing.container_name, self.listing.runtime_pid
        );
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                name,
                Phosphor::Flask(Weight::Regular).into(),
                ContainerRunState::Running.into(),
                Box::new(|_| {
                    vec![Section::new(
                        "Status",
                        dictionary_table([LabeledValue::new(
                            "State",
                            ContainerRunState::Running.label(),
                        )]),
                    )]
                }),
            )
        }
    }
}

pub struct ApptainerContainerRow<'a> {
    container: &'a ApptainerContainer,
}

impl<'a> ApptainerContainerRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let container = self.container;
        let name = format!(
            "{} pid {}",
            container.listing.container_name, container.listing.runtime_pid
        );
        ReadingRow {
            readout: IndicatorReadout {
                label: ReadoutLabel::Labeled(approximate_memory_text(container.memory_in_use)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                name,
                Phosphor::Flask(Weight::Regular).into(),
                IndicatorState::On,
                Box::new(move |clock| Self::hover_card(container, clock)),
            )
        }
    }

    fn hover_card(container: &ApptainerContainer, clock: Clock) -> Vec<Section> {
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
    type PowerRequest = i32;

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
    ) -> Vec<FamilyRow<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                sample: ApptainerContainerListingRow { listing }.reading_row(),
                power: RowPower::Stoppable(listing.runtime_pid),
            })
            .collect()
    }

    fn detail_rows(detail: &FamilyDetails<ApptainerContainer>) -> Vec<FamilyRow<'_, Self>> {
        detail
            .details
            .iter()
            .map(|container| FamilyRow {
                sample: ApptainerContainerRow { container }.reading_row(),
                power: RowPower::Stoppable(container.listing.runtime_pid),
            })
            .collect()
    }

    fn stop(
        runtime_pid: i32,
    ) -> impl Future<Output = Result<(), GuestPowerError>> + Send + 'static {
        ready(stop_apptainer_container(runtime_pid))
    }
}
