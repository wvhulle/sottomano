use chrono::Locale;
use situo_client::{Estimate, Origin, Place, Position};
use sottomano_sources::weather::situo_location_fix::Location;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
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

    fn glyph(self) -> Glyph {
        match self {
            Self::GoogleMaps => SimpleIcon::GoogleMaps.into(),
            Self::OpenStreetMap => SimpleIcon::OpenStreetMap.into(),
        }
    }

    fn url(self, position: Position) -> String {
        let Position {
            latitude,
            longitude,
        } = position;
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

impl BarWidget for Location {
    fn name(&self) -> &'static str {
        "Location"
    }

    fn icon(&self) -> Glyph {
        match self.geo.origin {
            Origin::Manual => Phosphor::MapPinLine(Weight::Regular).into(),
            Origin::Estimated(_) => Phosphor::MapPin(Weight::Regular).into(),
        }
    }

    fn fallback_icon() -> Glyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.geo.place.as_ref().map(|place| place.name.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let estimate = match &self.geo.origin {
            Origin::Manual => None,
            Origin::Estimated(estimate) => Some(estimate),
        };
        let position = self.geo.position;
        let position_section = Section::dictionary([
            KeyValue::new("Latitude", format!("{:.4}", position.latitude)),
            KeyValue::new("Longitude", format!("{:.4}", position.longitude)),
            KeyValue::new("Source", self.geo.origin.to_string()),
        ])
        .titled("Position");
        TooltipBody::sections([
            place_section(self.geo.place.as_ref()),
            position_section,
            estimate_section(estimate),
            map_section(position),
        ])
    }
}

fn place_section(place: Option<&Place>) -> Section {
    Section::dictionary(place.into_iter().flat_map(|place| {
        [
            KeyValue::new("Name", place.name.as_str()),
            KeyValue::new("Country", place.country.as_str()),
            KeyValue::new("Distance", format!("{:.2} km", place.distance_km)),
        ]
    }))
    .titled("Place")
}

fn estimate_section(estimate: Option<&Estimate>) -> Section {
    Section::dictionary(estimate.into_iter().flat_map(|estimate| {
        let observations = estimate.observations;
        [
            KeyValue::new("Accuracy", accuracy(estimate.accuracy_metres)),
            KeyValue::new(
                "Access points",
                format!(
                    "{} seen, {} submitted",
                    observations.access_points_seen, observations.access_points_submitted
                ),
            ),
            KeyValue::new(
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

fn map_section(position: Position) -> Section {
    let Position {
        latitude,
        longitude,
    } = position;
    Section::dictionary(MapService::ALL.map(|service| {
        KeyValue::new(
            service.name(),
            Cell::Link {
                label: format!("{latitude:.5}, {longitude:.5}"),
                url: service.url(position),
                glyph: Some(service.glyph()),
            },
        )
    }))
    .titled("Map")
}

fn accuracy(metres: f64) -> String {
    if metres < 1000.0 {
        format!("{metres:.0} m")
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}
