use std::{borrow::Cow, convert::identity};

use chrono::Locale;
use iced::Subscription;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    flatpak_sandboxes::{
        FlatpakProbe, FlatpakSandbox, FlatpakSandboxListing, FlatpakSandboxListings,
        FlatpakSandboxes,
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

fn running_state_text(running: bool) -> &'static str {
    if running { "running" } else { "stopped" }
}

struct FlatpakSandboxListingRow<'a> {
    listing: &'a FlatpakSandboxListing,
}

impl GroupItemReading for FlatpakSandboxListingRow<'_> {
    fn name(&self) -> &'static str {
        "Flatpak app"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.application_id.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        sandbox_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(running_state_text(self.listing.running).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.listing.running {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("App", self.listing.application_id.clone()),
            LabeledValue::new("State", running_state_text(self.listing.running)),
        ])
    }
}

struct FlatpakSandboxRow<'a> {
    sandbox: &'a FlatpakSandbox,
}

impl GroupItemReading for FlatpakSandboxRow<'_> {
    fn name(&self) -> &'static str {
        "Flatpak app"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.sandbox.application_id.clone().into()
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

#[derive(Default)]
pub struct FlatpakSandboxesGroup {
    probe: LatestSnapshot<FlatpakProbe, HidesFetchFailures>,
    listings: LatestSnapshot<FlatpakSandboxListings, HidesFetchFailures>,
    detail: LatestSnapshot<FlatpakSandboxes, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum FlatpakSandboxesMessage {
    Probe(SnapshotDelivery<FlatpakProbe>),
    Listings(SnapshotDelivery<FlatpakSandboxListings>),
    Sandboxes(SnapshotDelivery<FlatpakSandboxes>),
    Expansion(WidgetGroupExpansionMessage),
}

impl FlatpakSandboxesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<FlatpakSandboxesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<FlatpakProbe>(()).map(FlatpakSandboxesMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<FlatpakSandboxListings>(()))
                .map(FlatpakSandboxesMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<FlatpakSandboxes>(()))
                .map(FlatpakSandboxesMessage::Sandboxes),
        ])
    }

    pub fn update(&mut self, message: FlatpakSandboxesMessage, clock: Clock) {
        match message {
            FlatpakSandboxesMessage::Probe(delivery) => self.probe.receive(delivery, clock),
            FlatpakSandboxesMessage::Listings(delivery) => self.listings.receive(delivery, clock),
            FlatpakSandboxesMessage::Sandboxes(delivery) => {
                self.detail.receive(delivery, clock);
            }
            FlatpakSandboxesMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &FlatpakSandboxTarget, clock: Clock) -> Option<HoverCard> {
        if let Some(detail) = self.detail.get() {
            let sandbox = detail
                .sandboxes
                .iter()
                .find(|sandbox| sandbox.instance_id == target.instance_id)?;
            return self
                .detail
                .hover_card_with(FlatpakSandboxRow { sandbox }.hover_card(clock));
        }
        let listing = self
            .listings
            .get()?
            .listings
            .iter()
            .find(|listing| listing.instance_id == target.instance_id)?;
        self.listings
            .hover_card_with(FlatpakSandboxListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        let rows = self.sandbox_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::FlatpakSandboxes,
            &self.expansion,
            identity,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: sandbox_icon(),
            reading: self,
        })
    }

    fn sandbox_rows(&self) -> Vec<GroupItem<'_, WidgetGroupExpansionMessage>> {
        if let Some(detail) = self.detail.get() {
            return detail
                .sandboxes
                .iter()
                .map(|sandbox| {
                    GroupItem::reading(
                        FlatpakSandboxRow { sandbox },
                        FlatpakSandboxTarget::of_instance(Cow::Owned(sandbox.instance_id.clone())),
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
                            FlatpakSandboxListingRow { listing },
                            FlatpakSandboxTarget::of_instance(Cow::Owned(
                                listing.instance_id.clone(),
                            )),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GroupItemReading for FlatpakSandboxesGroup {
    fn name(&self) -> &'static str {
        "Flatpak apps"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Flatpak apps".into()
    }

    fn icon(&self) -> IconGlyph {
        sandbox_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| format!("{} running", probe.running_instance_count))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.running_instance_count > 0)
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
                        "Running apps",
                        probe.running_instance_count.to_string(),
                    )]
                })
                .unwrap_or_default(),
        )
    }
}
