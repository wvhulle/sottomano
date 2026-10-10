use situo_client::{Estimate, Origin, Place, Position};

use crate::{
    data::source::weather::Location,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
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

    fn url(self, position: Position) -> String {
        let Position {
            latitude,
            longitude,
        } = position;
        match self {
            Self::GoogleMaps => {
                format!("https://www.google.com/maps/search/?api=1&query={latitude:.6},{longitude:.6}")
            }
            Self::OpenStreetMap => format!(
                "https://www.openstreetmap.org/?mlat={latitude:.6}&mlon={longitude:.6}#map={OPENSTREETMAP_ZOOM}/{latitude:.6}/{longitude:.6}"
            ),
        }
    }
}

impl BarWidget for Location {
    fn icon(&self) -> Glyph {
        match self.geo.origin {
            Origin::Manual => Phosphor::MapPinLine(self.state().weight()).into(),
            Origin::Estimated(_) => Phosphor::MapPin(self.state().weight()).into(),
        }
    }

    fn fallback_icon() -> Glyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.geo.place.as_ref().map(|place| place.name.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        let estimate = match &self.geo.origin {
            Origin::Manual => None,
            Origin::Estimated(estimate) => Some(estimate),
        };
        TooltipBody::sections(&[
            place_section(self.geo.place.as_ref()),
            self.position_section(),
            estimate_section(estimate),
            map_section(self.geo.position),
        ])
    }
}

impl Location {
    fn position_section(&self) -> Section {
        let position = self.geo.position;
        Section::dictionary(
            "Position",
            [
                KeyValue::new("Latitude", format!("{:.4}", position.latitude)),
                KeyValue::new("Longitude", format!("{:.4}", position.longitude)),
                KeyValue::new("Source", self.geo.origin.to_string()),
            ],
        )
    }
}

fn place_section(place: Option<&Place>) -> Section {
    Section::dictionary(
        "Place",
        place.into_iter().flat_map(|place| {
            [
                KeyValue::new("Name", place.name.as_str()),
                KeyValue::new("Country", place.country.as_str()),
                KeyValue::new("Distance", format!("{:.2} km", place.distance_km)),
            ]
        }),
    )
}

fn estimate_section(estimate: Option<&Estimate>) -> Section {
    Section::dictionary(
        ("Estimate", "A Wi-Fi and cell fix through BeaconDB."),
        estimate.into_iter().flat_map(|estimate| {
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
        }),
    )
}

fn map_section(position: Position) -> Section {
    Section::dictionary(
        "Map",
        MapService::ALL
            .map(|service| KeyValue::new(service.name(), Cell::Link(service.url(position)))),
    )
}

fn accuracy(metres: f64) -> String {
    if metres < 1000.0 {
        format!("{metres:.0} m")
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}
