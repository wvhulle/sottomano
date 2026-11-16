use sottomano_desktop_sinks::{
    flatpak_sandboxes::stop_flatpak_instance, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
    flatpak_sandboxes::{FlatpakProbe, FlatpakSandbox, FlatpakSandboxListing},
};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
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
    widgets::{
        icon_glyph::IconGlyph,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

fn sandbox_icon() -> IconGlyph {
    IconGlyph::SimpleIcon(SimpleIcon::Flatpak)
}

pub struct FlatpakSandboxListingIndicator<'a> {
    listing: &'a FlatpakSandboxListing,
}

impl<'a> FlatpakSandboxListingIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            format!(
                "{} {}",
                self.listing.application_id, self.listing.instance_id
            ),
            sandbox_icon(),
            ContainerRunState::Running.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("App", self.listing.application_id.clone()),
                LabeledValue::new("State", ContainerRunState::Running.label()),
            ]),
        )]
    }
}

pub struct FlatpakSandboxIndicator<'a> {
    sandbox: &'a FlatpakSandbox,
}

impl<'a> FlatpakSandboxIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        let label = match self.sandbox.memory_in_use {
            Some(memory_in_use) => ReadoutLabel::Labeled(approximate_memory_text(memory_in_use)),
            None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            format!(
                "{} {}",
                self.sandbox.listing.application_id, self.sandbox.listing.instance_id
            ),
            sandbox_icon(),
            IndicatorState::On,
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        let sandbox = self.sandbox;
        let mut detail_entries = vec![LabeledValue::new(
            "App",
            sandbox.listing.application_id.clone(),
        )];
        if let Some(memory_in_use) = sandbox.memory_in_use {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(memory_in_use),
            ));
        }
        if let Some(uptime) = sandbox.started_at.and_then(|started_at| {
            clock
                .wall
                .duration_since(started_at)
                .inspect_err(|clock_skew| {
                    tracing::debug!(
                        "flatpak sandboxes: start time of {} lies ahead: {clock_skew}",
                        sandbox.listing.application_id
                    );
                })
                .ok()
        }) {
            detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
        }
        if let Some(runtime_reference) = &sandbox.runtime_reference {
            detail_entries.push(LabeledValue::new("Runtime", runtime_reference.clone()));
        }
        if let Some(branch) = &sandbox.branch {
            detail_entries.push(LabeledValue::new("Branch", branch.clone()));
        }
        detail_entries.extend([
            LabeledValue::new("Instance", sandbox.listing.instance_id.clone()),
            LabeledValue::new("Sandbox PID", sandbox.sandbox_pid.to_string()),
        ]);
        vec![Section::new("Sandbox", dictionary_table(detail_entries))]
    }
}

#[derive(Debug, Clone)]
pub struct FlatpakFamily;

pub type FlatpakSandboxesGroup = ContainerFamilyGroup<FlatpakFamily>;
pub type FlatpakSandboxesMessage = ContainerFamilyMessage<FlatpakFamily>;
pub type FlatpakSandboxesInteraction = ContainerFamilyInteraction<FlatpakFamily>;

impl ContainerFamily for FlatpakFamily {
    type Probe = FlatpakProbe;
    type Listings = FamilyListings<FlatpakSandboxListing>;
    type Detail = FamilyDetails<FlatpakSandbox>;
    type PowerRequest = String;

    const FAMILY: IndicatorGroupFamily = IndicatorGroupFamily::FlatpakSandboxes;
    const STOP_HINT: &'static str = "quit the app";
    const GROUP_NAME: &'static str = "Flatpak apps";

    fn group_glyph() -> IconGlyph {
        sandbox_icon()
    }

    fn running_count(probe: &FlatpakProbe) -> usize {
        probe.running_count
    }

    fn listing_indicators(
        listings: &FamilyListings<FlatpakSandboxListing>,
    ) -> Vec<FamilyIndicator<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyIndicator {
                indicator: FlatpakSandboxListingIndicator { listing }.indicator(),
                power: RowPower::Stoppable(listing.instance_id.clone()),
            })
            .collect()
    }

    fn detail_indicators(detail: &FamilyDetails<FlatpakSandbox>) -> Vec<FamilyIndicator<'_, Self>> {
        detail
            .details
            .iter()
            .map(|sandbox| FamilyIndicator {
                indicator: FlatpakSandboxIndicator { sandbox }.indicator(),
                power: RowPower::Stoppable(sandbox.listing.instance_id.clone()),
            })
            .collect()
    }

    async fn stop(instance_id: String) -> Result<(), GuestPowerError> {
        stop_flatpak_instance(&instance_id).await
    }
}
