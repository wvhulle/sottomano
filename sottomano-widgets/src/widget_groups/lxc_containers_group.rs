use std::{borrow::Cow, convert::identity};

use chrono::Locale;
use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    lxc_containers::{
        LxcContainer, LxcContainerListing, LxcContainerListings, LxcContainers, LxcProbe,
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

struct LxcContainerListingRow<'a> {
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

struct LxcContainerRow<'a> {
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

#[derive(Default)]
pub struct LxcContainersGroup {
    probe: LatestSnapshot<LxcProbe, HidesFetchFailures>,
    listings: LatestSnapshot<LxcContainerListings, HidesFetchFailures>,
    detail: LatestSnapshot<LxcContainers, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum LxcContainersMessage {
    Probe(SnapshotDelivery<LxcProbe>),
    Listings(SnapshotDelivery<LxcContainerListings>),
    Containers(SnapshotDelivery<LxcContainers>),
    Expansion(WidgetGroupExpansionMessage),
}

impl LxcContainersGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<LxcContainersMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<LxcProbe>(()).map(LxcContainersMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<LxcContainerListings>(()))
                .map(LxcContainersMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<LxcContainers>(()))
                .map(LxcContainersMessage::Containers),
        ])
    }

    pub fn update(&mut self, message: LxcContainersMessage, clock: Clock) {
        match message {
            LxcContainersMessage::Probe(delivery) => self.probe.receive(delivery, clock),
            LxcContainersMessage::Listings(delivery) => self.listings.receive(delivery, clock),
            LxcContainersMessage::Containers(delivery) => self.detail.receive(delivery, clock),
            LxcContainersMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &LxcContainerTarget, clock: Clock) -> Option<HoverCard> {
        if let Some(detail) = self.detail.get() {
            let container = detail
                .containers
                .iter()
                .find(|container| container.container_name == target.container_name)?;
            return self
                .detail
                .hover_card_with(LxcContainerRow { container }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.container_name == target.container_name)?;
        self.listings
            .hover_card_with(LxcContainerListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let rows = self.container_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::LxcContainers,
            &self.expansion,
            identity,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Package(Weight::Regular).into(),
            reading: self,
        })
    }

    fn container_rows(&self) -> Vec<GroupItem<'_, WidgetGroupExpansionMessage>> {
        if let Some(detail) = self.detail.get() {
            return detail
                .containers
                .iter()
                .map(|container| {
                    GroupItem::reading(
                        LxcContainerRow { container },
                        LxcContainerTarget::of_container(Cow::Owned(
                            container.container_name.clone(),
                        )),
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
                            LxcContainerListingRow { listing },
                            LxcContainerTarget::of_container(Cow::Owned(
                                listing.container_name.clone(),
                            )),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GroupItemReading for LxcContainersGroup {
    fn name(&self) -> &'static str {
        "LXC containers"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "LXC containers".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe.get().map(|probe| {
            format!(
                "{} running, {} configured",
                probe.running_count, probe.configured_count
            )
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.probe
            .get()
            .filter(|probe| probe.running_count > 0)
            .map_or(ReadingDisplayState::Off, |_| ReadingDisplayState::On)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe
                .get()
                .map(|probe| {
                    vec![
                        LabeledValue::new("Running", probe.running_count.to_string()),
                        LabeledValue::new("Configured", probe.configured_count.to_string()),
                    ]
                })
                .unwrap_or_default(),
        )
    }
}
