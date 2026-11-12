use std::borrow::Cow;

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::waydroid_container::stop_waydroid_container;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    waydroid_container::{WaydroidListing, WaydroidProbe, WaydroidReadout, WaydroidState},
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
        value_cells::LabeledValue,
    },
};

const STOP_HINT: &str = "stop the Android container";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaydroidContainerTarget;

impl From<WaydroidContainerTarget> for HoverTarget {
    fn from(target: WaydroidContainerTarget) -> Self {
        HoverTarget::WaydroidContainer(target)
    }
}

struct WaydroidListingRow<'a> {
    listing: &'a WaydroidListing,
}

impl GroupItemReading for WaydroidListingRow<'_> {
    fn name(&self) -> &'static str {
        "Android"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Android".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Android)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(waydroid_state_text(&self.listing.state))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        waydroid_state_display(&self.listing.state)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mut detail_entries = vec![LabeledValue::new(
            "Container",
            if self.listing.container_running {
                "running"
            } else {
                "stopped"
            },
        )];
        if let WaydroidState::Session {
            session_state,
            user_name,
            lcd_density,
        } = &self.listing.state
        {
            detail_entries.push(LabeledValue::new("Session", session_state.clone()));
            if let Some(user_name) = user_name {
                detail_entries.push(LabeledValue::new("User", user_name.clone()));
            }
            if let Some(lcd_density) = lcd_density {
                detail_entries.push(LabeledValue::new("Screen density", lcd_density.clone()));
            }
        }
        HoverCardBody::dictionary(detail_entries)
    }
}

fn waydroid_state_text(state: &WaydroidState) -> String {
    match state {
        WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
        WaydroidState::ContainerOnly => "no session".to_owned(),
        WaydroidState::Absent => "stopped".to_owned(),
    }
}

fn waydroid_state_display(state: &WaydroidState) -> ReadingDisplayState {
    match state {
        WaydroidState::Session { .. } => ReadingDisplayState::On,
        WaydroidState::ContainerOnly => ReadingDisplayState::Idle,
        WaydroidState::Absent => ReadingDisplayState::Off,
    }
}

struct WaydroidContainerRow<'a> {
    readout: &'a WaydroidReadout,
}

impl GroupItemReading for WaydroidContainerRow<'_> {
    fn name(&self) -> &'static str {
        "Android"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Android".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Android)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match &self.readout.state {
            WaydroidState::Session { session_state, .. } => session_state.to_lowercase(),
            WaydroidState::ContainerOnly => "no session".to_owned(),
            WaydroidState::Absent => "stopped".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match &self.readout.state {
            WaydroidState::Session { .. } => ReadingDisplayState::On,
            WaydroidState::ContainerOnly => ReadingDisplayState::Idle,
            WaydroidState::Absent => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mut detail_entries = vec![LabeledValue::new(
            "Container",
            if self.readout.container_running {
                "running"
            } else {
                "stopped"
            },
        )];
        if let Some(memory_in_use) = self.readout.memory_in_use {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(memory_in_use),
            ));
        }
        if let WaydroidState::Session {
            session_state,
            user_name,
            lcd_density,
        } = &self.readout.state
        {
            detail_entries.push(LabeledValue::new("Session", session_state.clone()));
            if let Some(user_name) = user_name {
                detail_entries.push(LabeledValue::new("User", user_name.clone()));
            }
            if let Some(lcd_density) = lcd_density {
                detail_entries.push(LabeledValue::new("Screen density", lcd_density.clone()));
            }
        }
        HoverCardBody::dictionary(detail_entries)
    }
}

#[derive(Default)]
pub struct WaydroidContainerGroup {
    probe: LatestSnapshot<WaydroidProbe, HidesFetchFailures>,
    listing: LatestSnapshot<WaydroidListing, HidesFetchFailures>,
    detail: LatestSnapshot<WaydroidReadout, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum WaydroidContainerMessage {
    Probe(SnapshotDelivery<WaydroidProbe>),
    Listing(SnapshotDelivery<WaydroidListing>),
    Readout(SnapshotDelivery<WaydroidReadout>),
    StopRefused(String),
    Interaction(WaydroidContainerInteraction),
}

#[derive(Debug, Clone)]
pub enum WaydroidContainerInteraction {
    StopContainer,
    Expansion(WidgetGroupExpansionMessage),
}

impl WaydroidContainerGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<WaydroidContainerMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<WaydroidProbe>(()).map(WaydroidContainerMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<WaydroidListing>(buses.clone()))
                .map(WaydroidContainerMessage::Listing),
            demand
                .while_rows_shown(snapshot_subscription::<WaydroidReadout>(buses.clone()))
                .map(WaydroidContainerMessage::Readout),
        ])
    }

    pub fn update(
        &mut self,
        message: WaydroidContainerMessage,
        clock: Clock,
    ) -> Task<WaydroidContainerMessage> {
        match message {
            WaydroidContainerMessage::Probe(delivery) => {
                self.probe.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::Listing(delivery) => {
                self.listing.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::Readout(delivery) => {
                self.detail.receive(delivery, clock);
                Task::none()
            }
            WaydroidContainerMessage::StopRefused(refusal) => {
                tracing::warn!("{refusal}");
                Task::none()
            }
            WaydroidContainerMessage::Interaction(interaction) => self.interact(interaction),
        }
    }

    fn interact(
        &mut self,
        interaction: WaydroidContainerInteraction,
    ) -> Task<WaydroidContainerMessage> {
        match interaction {
            WaydroidContainerInteraction::StopContainer => {
                Task::future(async { stop_waydroid_container().await.err() })
                    .and_then(|refusal| Task::done(WaydroidContainerMessage::StopRefused(refusal)))
            }
            WaydroidContainerInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn shown_readout(&self) -> Option<&WaydroidReadout> {
        self.detail
            .get()
            .filter(|readout| readout.state != WaydroidState::Absent)
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        _target: &WaydroidContainerTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        if let Some(readout) = self.shown_readout() {
            return self
                .detail
                .hover_card_with(WaydroidContainerRow { readout }.hover_card(clock));
        }
        let listing = self
            .listing
            .get()
            .filter(|listing| listing.state != WaydroidState::Absent)?;
        self.listing
            .hover_card_with(WaydroidListingRow { listing }.hover_card(clock))
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WaydroidContainerInteraction> {
        let rows = self.container_rows();
        WidgetGroupContent::expandable(
            WidgetGroupFamily::WaydroidContainer,
            &self.expansion,
            WaydroidContainerInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: IconGlyph::SimpleIcon(SimpleIcon::Android),
            reading: self,
        })
    }

    fn container_rows(&self) -> Vec<GroupItem<'_, WaydroidContainerInteraction>> {
        if let Some(readout) = self.shown_readout() {
            let row = GroupItem::reading(WaydroidContainerRow { readout }, WaydroidContainerTarget);
            let row = if readout.container_running {
                row.on_right_press(STOP_HINT, WaydroidContainerInteraction::StopContainer)
            } else {
                row
            };
            return vec![row];
        }
        if let Some(listing) = self
            .listing
            .get()
            .filter(|listing| listing.container_running)
        {
            return vec![GroupItem::reading(
                WaydroidListingRow { listing },
                WaydroidContainerTarget,
            )];
        }
        Vec::new()
    }
}

impl GroupItemReading for WaydroidContainerGroup {
    fn name(&self) -> &'static str {
        "Android"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Android".into()
    }

    fn icon(&self) -> IconGlyph {
        IconGlyph::SimpleIcon(SimpleIcon::Android)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe
            .get()
            .map(|probe| presence_text(probe.0).to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self
            .probe
            .get()
            .is_some_and(|probe| probe.0 == Availability::Available)
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
                        "Present",
                        presence_text(probe.0).to_owned(),
                    )]
                })
                .unwrap_or_default(),
        )
    }
}

fn presence_text(presence: Availability) -> &'static str {
    match presence {
        Availability::Available => "present",
        Availability::Unavailable => "stopped",
    }
}
