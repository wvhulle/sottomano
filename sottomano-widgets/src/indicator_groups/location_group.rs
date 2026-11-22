mod estimate_face;
mod place_face;
mod radio_face;
mod updates_face;

use std::iter::once;

use iced::{Subscription, Task, futures::SinkExt};
use situo_client::{FixOrigin, LocationRequest, LocationRequests};
use sottomano_desktop_sources::{SnapshotDelivery, weather::situo_location_fix::Location};

use crate::{
    background_work::{logged_failure_task, snapshot_subscription},
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        location_group::{
            estimate_face::EstimateFace, place_face::PlaceFace, radio_face::RadioFace,
            updates_face::UpdatesFace,
        },
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
        let place_row = PlaceFace { location }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.location
                    .hover_card_with(PlaceFace { location }.hover_card())
            }));
        let updates_row = UpdatesFace { location }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.location
                    .hover_card_with(UpdatesFace { location }.hover_card())
            }));
        let estimate_rows = match &location.geo.origin {
            FixOrigin::Manual | FixOrigin::Pinned => Vec::new(),
            FixOrigin::Estimated(estimate) => {
                let published = location.geo.position;
                vec![
                    EstimateFace {
                        estimate,
                        published,
                    }
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.location.hover_card_with(
                            EstimateFace {
                                estimate,
                                published,
                            }
                            .hover_card(),
                        )
                    })),
                    RadioFace { estimate }
                        .indicator()
                        .with_hover_card(Box::new(move |_| {
                            self.location
                                .hover_card_with(RadioFace { estimate }.hover_card())
                        })),
                ]
            }
        };
        once(place_row)
            .chain(estimate_rows)
            .chain(once(updates_row))
            .map(|row| row.refreshed_by(LocationInteraction::LookUpAgain))
            .collect()
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        let rows = match self.location.get() {
            Some(location) => self.location_indicators(location),
            None => self
                .location
                .failure()
                .map(|failure_placeholder| {
                    failure_placeholder
                        .indicator(LOCATION_GLYPH)
                        .with_hover_card(Box::new(move |_| {
                            self.location.sections_described_by(|_| Vec::new())
                        }))
                        .refreshed_by(LocationInteraction::LookUpAgain)
                })
                .into_iter()
                .collect(),
        };
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Location,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Location.title(),
                LOCATION_GLYPH,
                IndicatorState::Idle,
            ),
            &self.expansion,
            LocationInteraction::Expansion,
            rows,
        )
    }
}
