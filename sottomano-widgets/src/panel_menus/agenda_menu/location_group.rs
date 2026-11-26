mod estimate_indicator;
mod place_indicator;
mod radio_indicator;
mod updates_indicator;

use std::iter::once;

use iced::{Subscription, Task, futures::SinkExt};
use situo_client::{LocationRequest, LocationRequests};
use sottomano_desktop_sources::{SnapshotDelivery, weather::situo_location_fix::Location};

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const LOCATION_GLYPH: IconGlyph = IconGlyph::Phosphor(Phosphor::MapPin(Weight::Regular));

#[derive(Debug, Clone, Copy)]
pub enum LocationInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    LookUpAgain,
}

#[derive(Debug)]
pub enum LocationMessage {
    Location(SnapshotDelivery<Location>),
}

#[derive(Default)]
pub struct LocationGroup {
    location: SubscribedSnapshot<Location>,
    expansion: IndicatorGroupExpansion,
}

impl LocationGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self) -> Subscription<LocationMessage> {
        snapshot_subscription::<Location>(()).map(LocationMessage::Location)
    }

    pub fn update(&mut self, message: LocationMessage, clock: Clock) -> Task<LocationMessage> {
        let LocationMessage::Location(delivery) = message;
        self.location.receive(delivery, clock);
        Task::none()
    }

    pub fn interact(&mut self, interaction: LocationInteraction) -> Task<LocationMessage> {
        match interaction {
            LocationInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            LocationInteraction::LookUpAgain => {
                logged_failure_task("request a situo lookup".to_owned(), async {
                    LocationRequests::default()
                        .send(LocationRequest::LookUpAgain)
                        .await
                })
            }
        }
    }

    fn location_indicators<'view>(
        &'view self,
        location: &'view Location,
    ) -> Vec<GroupIndicator<'view>> {
        once(self.place_row(location))
            .chain(self.estimate_rows(location))
            .chain(self.radio_rows(location))
            .chain(once(self.updates_row(location)))
            .map(|row| row.refreshed_by(LocationInteraction::LookUpAgain))
            .collect()
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Location,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Location.title(),
                LOCATION_GLYPH,
                IndicatorState::Idle,
            ),
            &self.expansion,
            LocationInteraction::Expansion,
            self.location
                .indicator_listing(
                    |location| self.location_indicators(location),
                    LOCATION_GLYPH,
                )
                .map_indicators(|location_row| {
                    location_row.refreshed_by(LocationInteraction::LookUpAgain)
                }),
        )
    }
}
