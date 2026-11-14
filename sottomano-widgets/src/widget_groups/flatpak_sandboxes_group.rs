use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sinks::flatpak_sandboxes::stop_flatpak_instance;
use sottomano_desktop_sources::{
    container_run_state::ContainerRunState,
    flatpak_sandboxes::{
        FlatpakProbe, FlatpakSandbox, FlatpakSandboxListing, FlatpakSandboxListings,
        FlatpakSandboxes,
    },
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
    instance_id: Cow<'static, str>,
}

impl FlatpakSandboxTarget {
    #[must_use]
    pub const fn of_instance(instance_id: Cow<'static, str>) -> Self {
        Self { instance_id }
    }
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

impl GroupItemReading for FlatpakSandboxListingRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        format!(
            "{} {}",
            self.listing.application_id, self.listing.instance_id
        )
        .into()
    }

    fn icon(&self) -> IconGlyph {
        sandbox_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(ContainerRunState::Running.label().to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ContainerRunState::Running.into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("App", self.listing.application_id.clone()),
            LabeledValue::new("State", ContainerRunState::Running.label()),
        ])
    }
}

pub struct FlatpakSandboxRow<'a> {
    sandbox: &'a FlatpakSandbox,
}

impl GroupItemReading for FlatpakSandboxRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        format!(
            "{} {}",
            self.sandbox.application_id, self.sandbox.instance_id
        )
        .into()
    }

    fn icon(&self) -> IconGlyph {
        sandbox_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.sandbox.memory_in_use.map(approximate_memory_text)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::On
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let sandbox = self.sandbox;
        let mut detail_entries = vec![LabeledValue::new("App", sandbox.application_id.clone())];
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
                        sandbox.application_id
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
            LabeledValue::new("Instance", sandbox.instance_id.clone()),
            LabeledValue::new("Sandbox PID", sandbox.sandbox_pid.to_string()),
        ]);
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Debug, Clone)]
pub struct FlatpakFamily;

pub type FlatpakSandboxesGroup = ContainerFamilyGroup<FlatpakFamily>;
pub type FlatpakSandboxesMessage = ContainerFamilyMessage<FlatpakFamily>;
pub type FlatpakSandboxesInteraction = ContainerFamilyInteraction<FlatpakFamily>;

impl ContainerFamily for FlatpakFamily {
    type Probe = FlatpakProbe;
    type Listings = FlatpakSandboxListings;
    type Detail = FlatpakSandboxes;
    type Target = FlatpakSandboxTarget;
    type PowerRequest = String;
    type ListingRow<'a> = FlatpakSandboxListingRow<'a>;
    type DetailRow<'a> = FlatpakSandboxRow<'a>;

    const FAMILY: WidgetGroupFamily = WidgetGroupFamily::FlatpakSandboxes;
    const STOP_HINT: &'static str = "quit the app";
    const GROUP_NAME: &'static str = "Flatpak apps";

    fn group_glyph() -> IconGlyph {
        sandbox_icon()
    }

    fn running_count(probe: &FlatpakProbe) -> usize {
        probe.running_instance_count
    }

    fn listing_rows(
        listings: &FlatpakSandboxListings,
    ) -> Vec<FamilyRow<Self, FlatpakSandboxListingRow<'_>>> {
        listings
            .listings
            .iter()
            .map(|listing| FamilyRow {
                target: FlatpakSandboxTarget::of_instance(Cow::Owned(listing.instance_id.clone())),
                reading: FlatpakSandboxListingRow { listing },
                power: RowPower::Stoppable(listing.instance_id.clone()),
            })
            .collect()
    }

    fn detail_rows(detail: &FlatpakSandboxes) -> Vec<FamilyRow<Self, FlatpakSandboxRow<'_>>> {
        detail
            .sandboxes
            .iter()
            .map(|sandbox| FamilyRow {
                target: FlatpakSandboxTarget::of_instance(Cow::Owned(sandbox.instance_id.clone())),
                reading: FlatpakSandboxRow { sandbox },
                power: RowPower::Stoppable(sandbox.instance_id.clone()),
            })
            .collect()
    }

    fn group_hover_entries(probe: &FlatpakProbe) -> Vec<LabeledValue> {
        vec![LabeledValue::new(
            "Running apps",
            probe.running_instance_count.to_string(),
        )]
    }

    async fn stop(instance_id: String) -> Result<(), String> {
        stop_flatpak_instance(&instance_id).await
    }
}
