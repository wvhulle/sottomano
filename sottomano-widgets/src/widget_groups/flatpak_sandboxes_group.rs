use std::borrow::Cow;

use chrono::Locale;
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
        process_usage_cells::approximate_memory_text,
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandboxTarget {
    pub instance_id: Cow<'static, str>,
}

impl From<FlatpakSandboxTarget> for HoverTarget {
    fn from(target: FlatpakSandboxTarget) -> Self {
        HoverTarget::FlatpakSandbox(target)
    }
}

fn sandbox_icon() -> IconGlyph {
    IconGlyph::SimpleIcon(SimpleIcon::Flatpak)
}

pub struct FlatpakSandboxListingRow<'a> {
    listing: &'a FlatpakSandboxListing,
}

impl<'a> FlatpakSandboxListingRow<'a> {
    fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(ContainerRunState::Running.label().to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!(
                    "{} {}",
                    self.listing.application_id, self.listing.instance_id
                ),
                sandbox_icon(),
                ContainerRunState::Running.into(),
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("App", self.listing.application_id.clone()),
                LabeledValue::new("State", ContainerRunState::Running.label()),
            ]),
        )]
    }
}

pub struct FlatpakSandboxRow<'a> {
    sandbox: &'a FlatpakSandbox,
}

impl<'a> FlatpakSandboxRow<'a> {
    fn reading_row(self, _clock: Clock, _locale: Locale) -> ReadingRow<'a> {
        let label = match self.sandbox.memory_in_use {
            Some(memory_in_use) => ReadoutLabel::Labeled(approximate_memory_text(memory_in_use)),
            None => ReadoutLabel::Unlabeled,
        };
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!(
                    "{} {}",
                    self.sandbox.listing.application_id, self.sandbox.listing.instance_id
                ),
                sandbox_icon(),
                ReadingDisplayState::On,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
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
    type Target = FlatpakSandboxTarget;
    type PowerRequest = String;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::FlatpakSandboxes;
    const STOP_HINT: &'static str = "quit the app";
    const GROUP_NAME: &'static str = "Flatpak apps";

    fn group_glyph() -> IconGlyph {
        sandbox_icon()
    }

    fn running_count(probe: &FlatpakProbe) -> usize {
        probe.running_count
    }

    fn listing_rows(
        listings: &FamilyListings<FlatpakSandboxListing>,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: FlatpakSandboxTarget {
                    instance_id: Cow::Owned(listing.instance_id.clone()),
                },
                reading: FlatpakSandboxListingRow { listing }.reading_row(clock, locale),
                power: RowPower::Stoppable(listing.instance_id.clone()),
            })
            .collect()
    }

    fn detail_rows(
        detail: &FamilyDetails<FlatpakSandbox>,
        clock: Clock,
        locale: Locale,
    ) -> Vec<FamilyRow<'_, Self>> {
        detail
            .details
            .iter()
            .map(|sandbox| FamilyRow {
                target: FlatpakSandboxTarget {
                    instance_id: Cow::Owned(sandbox.listing.instance_id.clone()),
                },
                reading: FlatpakSandboxRow { sandbox }.reading_row(clock, locale),
                power: RowPower::Stoppable(sandbox.listing.instance_id.clone()),
            })
            .collect()
    }

    fn group_hover_entries(probe: &FlatpakProbe) -> Vec<LabeledValue> {
        vec![LabeledValue::new(
            "Running apps",
            probe.running_count.to_string(),
        )]
    }

    async fn stop(instance_id: String) -> Result<(), GuestPowerError> {
        stop_flatpak_instance(&instance_id).await
    }
}
