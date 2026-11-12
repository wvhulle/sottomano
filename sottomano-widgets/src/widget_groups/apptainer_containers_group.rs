use std::{borrow::Cow, convert::identity};

use chrono::Locale;
use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    apptainer_containers::{
        ApptainerContainer, ApptainerContainerListing, ApptainerContainerListings,
        ApptainerContainers, ApptainerProbe,
    },
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::approximate_memory_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
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

struct ApptainerContainerListingRow<'a> {
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

fn running_state_text(running: bool) -> &'static str {
    if running { "running" } else { "stopped" }
}

fn running_display_state(running: bool) -> ReadingDisplayState {
    if running {
        ReadingDisplayState::On
    } else {
        ReadingDisplayState::Off
    }
}

struct ApptainerContainerRow<'a> {
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
        if let Ok(uptime) = clock.wall.duration_since(container.started_at) {
            detail_entries.push(LabeledValue::new("Uptime", ValueCell::duration(uptime)));
        }
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

#[derive(Default)]
pub struct ApptainerContainersGroup {
    probe: LatestSnapshot<ApptainerProbe, HidesFetchFailures>,
    listings: LatestSnapshot<ApptainerContainerListings, HidesFetchFailures>,
    detail: LatestSnapshot<ApptainerContainers, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ApptainerContainersMessage {
    Probe(SnapshotDelivery<ApptainerProbe>),
    Listings(SnapshotDelivery<ApptainerContainerListings>),
    Containers(SnapshotDelivery<ApptainerContainers>),
    Expansion(WidgetGroupExpansionMessage),
}

impl ApptainerContainersGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
    ) -> Subscription<ApptainerContainersMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<ApptainerProbe>(()).map(ApptainerContainersMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<ApptainerContainerListings>(()))
                .map(ApptainerContainersMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<ApptainerContainers>(()))
                .map(ApptainerContainersMessage::Containers),
        ])
    }

    pub fn update(&mut self, message: ApptainerContainersMessage, clock: Clock) {
        match message {
            ApptainerContainersMessage::Probe(delivery) => {
                self.probe.receive(delivery, clock);
            }
            ApptainerContainersMessage::Listings(delivery) => {
                self.listings.receive(delivery, clock);
            }
            ApptainerContainersMessage::Containers(delivery) => {
                self.detail.receive(delivery, clock);
            }
            ApptainerContainersMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
    }

    fn listed_containers(&self) -> &[ApptainerContainer] {
        self.detail
            .get()
            .map(|listing| listing.containers.as_slice())
            .unwrap_or_default()
    }

    fn listed_names(&self) -> &[ApptainerContainerListing] {
        if self.listed_containers().is_empty() {
            return self
                .listings
                .get()
                .map(|listings| listings.listings.as_slice())
                .unwrap_or_default();
        }
        &[]
    }

    fn container_rows(&self) -> Vec<GroupItem<'_, WidgetGroupExpansionMessage>> {
        let detailed = self.listed_containers();
        if !detailed.is_empty() {
            return detailed
                .iter()
                .map(|container| {
                    GroupItem::reading(
                        ApptainerContainerRow { container },
                        ApptainerContainerTarget::of_runtime(container.runtime_pid),
                    )
                })
                .collect();
        }
        self.listings
            .get()
            .map(|listings| {
                listings
                    .listings
                    .iter()
                    .map(|listing| {
                        GroupItem::reading(
                            ApptainerContainerListingRow { listing },
                            ApptainerContainerTarget::of_runtime(listing.runtime_pid),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: &ApptainerContainerTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        if let Some(container) = self
            .listed_containers()
            .iter()
            .find(|container| container.runtime_pid == target.runtime_pid)
        {
            return self
                .detail
                .hover_card_with(ApptainerContainerRow { container }.hover_card(clock));
        }
        let listing = self
            .listed_names()
            .iter()
            .find(|listing| listing.runtime_pid == target.runtime_pid)?;
        self.listings
            .hover_card_with(ApptainerContainerListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let rows = self.container_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::ApptainerContainers,
            &self.expansion,
            identity,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Flask(Weight::Regular).into(),
            reading: self,
        })
    }
}

impl GroupItemReading for ApptainerContainersGroup {
    fn name(&self) -> &'static str {
        "Apptainer containers"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Apptainer containers".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Flask(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| format!("{} running", probe.running_count))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.running_count > 0)
        {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe
                .get()
                .map(|probe| {
                    vec![LabeledValue::new(
                        "Running",
                        probe.running_count.to_string(),
                    )]
                })
                .unwrap_or_default(),
        )
    }
}
