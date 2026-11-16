use situo_client::{BeaconDbEstimate, FixOrigin, GeoCoordinates, NearestPlace};
use sottomano_desktop_sources::{SnapshotSource, weather::situo_location_fix::Location};
use uom::si::length::kilometer;

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const OPENSTREETMAP_ZOOM: u8 = 15;

#[derive(Debug, Clone, Copy)]
enum MapService {
    GoogleMaps,
    OpenStreetMap,
}

impl MapService {
    const ALL: [Self; 2] = [Self::GoogleMaps, Self::OpenStreetMap];

    const fn name(self) -> &'static str {
        match self {
            Self::GoogleMaps => "Google Maps",
            Self::OpenStreetMap => "OpenStreetMap",
        }
    }

    fn glyph(self) -> IconGlyph {
        match self {
            Self::GoogleMaps => SimpleIcon::GoogleMaps.into(),
            Self::OpenStreetMap => SimpleIcon::OpenStreetMap.into(),
        }
    }

    fn url(self, position: GeoCoordinates) -> String {
        let latitude = position.latitude_degrees();
        let longitude = position.longitude_degrees();
        match self {
            Self::GoogleMaps => {
                format!(
                    "https://www.google.com/maps/search/?api=1&query={latitude:.6},{longitude:.6}"
                )
            }
            Self::OpenStreetMap => format!(
                "https://www.openstreetmap.org/?mlat={latitude:.6}&mlon={longitude:.6}#map={OPENSTREETMAP_ZOOM}/{latitude:.6}/{longitude:.6}"
            ),
        }
    }
}

pub(crate) const LOCATION_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::MapPin(Weight::Regular));

pub fn location_indicator(location: &Location) -> GroupIndicator<'_> {
    let glyph = match location.geo.origin {
        FixOrigin::Manual => Phosphor::MapPinLine(Weight::Regular).into(),
        FixOrigin::Pinned => Phosphor::PushPin(Weight::Regular).into(),
        FixOrigin::Estimated(_) => Phosphor::MapPin(Weight::Regular).into(),
    };
    let label = match &location.geo.place {
        Some(place) if !place.name.is_empty() => ReadoutLabel::Labeled(place.name.clone()),
        Some(_) | None => ReadoutLabel::Unlabeled,
    };
    GroupIndicator::glyph_only(Location::NAME, glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| location_hover_card(location)))
}

pub fn location_hover_card(location: &Location) -> Vec<Section> {
    let estimate = match &location.geo.origin {
        FixOrigin::Manual | FixOrigin::Pinned => None,
        FixOrigin::Estimated(estimate) => Some(estimate),
    };
    let position = location.geo.position;
    let position_section = Section::new(
        "Position",
        dictionary_table([
            LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(4)),
            LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(4)),
            LabeledValue::new("Source", location.geo.origin.to_string()),
        ]),
    );
    vec![
        place_section(location.geo.place.as_ref()),
        position_section,
        estimate_section(estimate),
        map_section(position),
    ]
}

fn place_section(place: Option<&NearestPlace>) -> Section {
    Section::new(
        "Place",
        dictionary_table(place.into_iter().flat_map(|place| {
            [
                LabeledValue::new("Name", place.name.as_str()),
                LabeledValue::new("Country", place.country.as_str()),
                LabeledValue::new(
                    "Distance",
                    Quantity::measured(
                        place.distance.get::<kilometer>(),
                        DisplayUnit::Kilometer,
                        2,
                    ),
                ),
            ]
        })),
    )
}

fn estimate_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    Section::new(
        SectionTitle::captioned("Estimate", "A Wi-Fi and cell fix through BeaconDB."),
        dictionary_table(estimate.into_iter().flat_map(|estimate| {
            let observations = estimate.observations;
            [
                LabeledValue::new("Accuracy", Quantity::distance(estimate.accuracy)),
                LabeledValue::new(
                    "Access points",
                    format!(
                        "{} seen, {} submitted",
                        observations.access_points_seen, observations.access_points_submitted
                    ),
                ),
                LabeledValue::new(
                    "Cells",
                    format!(
                        "{} seen, {} submitted",
                        observations.cells_seen, observations.cells_submitted
                    ),
                ),
            ]
        })),
    )
}

fn map_section(position: GeoCoordinates) -> Section {
    let latitude = position.latitude_degrees();
    let longitude = position.longitude_degrees();
    Section::new(
        "Map",
        dictionary_table(MapService::ALL.map(|service| {
            LabeledValue::new(
                service.name(),
                ValueCell::BrandedLink {
                    label: format!("{latitude:.5}, {longitude:.5}"),
                    url: service.url(position),
                    glyph: service.glyph(),
                },
            )
        })),
    )
}
