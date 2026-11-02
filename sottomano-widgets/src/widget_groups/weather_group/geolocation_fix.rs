use chrono::Locale;
use situo_client::{BeaconDbEstimate, FixOrigin, GeoCoordinates, NearestPlace};
use sottomano_sources::weather::situo_location_fix::Location;
use uom::si::length::kilometer;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
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

impl GroupItemReading for Location {
    fn name(&self) -> &'static str {
        "Location"
    }

    fn icon(&self) -> IconGlyph {
        match self.geo.origin {
            FixOrigin::Manual => Phosphor::MapPinLine(Weight::Regular).into(),
            FixOrigin::Pinned => Phosphor::PushPin(Weight::Regular).into(),
            FixOrigin::Estimated(_) => Phosphor::MapPin(Weight::Regular).into(),
        }
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.geo.place.as_ref().map(|place| place.name.clone())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let estimate = match &self.geo.origin {
            FixOrigin::Manual | FixOrigin::Pinned => None,
            FixOrigin::Estimated(estimate) => Some(estimate),
        };
        let position = self.geo.position;
        let position_section = Section::dictionary([
            LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(4)),
            LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(4)),
            LabeledValue::new("Source", self.geo.origin.to_string()),
        ])
        .titled("Position");
        HoverCardBody::sections([
            place_section(self.geo.place.as_ref()),
            position_section,
            estimate_section(estimate),
            map_section(position),
        ])
    }
}

fn place_section(place: Option<&NearestPlace>) -> Section {
    Section::dictionary(place.into_iter().flat_map(|place| {
        [
            LabeledValue::new("Name", place.name.as_str()),
            LabeledValue::new("Country", place.country.as_str()),
            LabeledValue::new(
                "Distance",
                Quantity::measured(place.distance.get::<kilometer>(), DisplayUnit::Kilometer, 2),
            ),
        ]
    }))
    .titled("Place")
}

fn estimate_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    Section::dictionary(estimate.into_iter().flat_map(|estimate| {
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
    }))
    .titled(("Estimate", "A Wi-Fi and cell fix through BeaconDB."))
}

fn map_section(position: GeoCoordinates) -> Section {
    let latitude = position.latitude_degrees();
    let longitude = position.longitude_degrees();
    Section::dictionary(MapService::ALL.map(|service| {
        LabeledValue::new(
            service.name(),
            ValueCell::BrandedLink {
                label: format!("{latitude:.5}, {longitude:.5}"),
                url: service.url(position),
                glyph: service.glyph(),
            },
        )
    }))
    .titled("Map")
}
