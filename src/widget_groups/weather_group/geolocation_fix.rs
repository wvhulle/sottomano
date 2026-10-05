use situo_client::{Estimate, Origin, Place, Position};

use crate::{
    data::source::weather::Location,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody, link, row, section},
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
        let place = self.geo.place.iter().map(place_section);
        let estimate = match &self.geo.origin {
            Origin::Manual => None,
            Origin::Estimated(estimate) => Some(estimate_section(estimate)),
        };
        TooltipBody::Sections(
            place
                .chain([self.position_section()])
                .chain(estimate)
                .chain([map_section(self.geo.position)])
                .collect(),
        )
    }
}

impl Location {
    fn position_section(&self) -> Section {
        let position = self.geo.position;
        section(
            "Position",
            vec![
                row("Latitude", format!("{:.4}", position.latitude)),
                row("Longitude", format!("{:.4}", position.longitude)),
                row("Source", self.geo.origin.to_string()),
            ],
        )
    }
}

fn place_section(place: &Place) -> Section {
    section(
        "Place",
        vec![
            row("Name", place.name.clone()),
            row("Country", place.country.clone()),
            row("Distance", format!("{:.2} km", place.distance_km)),
        ],
    )
}

fn estimate_section(estimate: &Estimate) -> Section {
    let observations = estimate.observations;
    section(
        "Estimate",
        vec![
            row("Accuracy", accuracy(estimate.accuracy_metres)),
            row(
                "Access points",
                format!(
                    "{} seen, {} submitted",
                    observations.access_points_seen, observations.access_points_submitted
                ),
            ),
            row(
                "Cells",
                format!(
                    "{} seen, {} submitted",
                    observations.cells_seen, observations.cells_submitted
                ),
            ),
        ],
    )
}

fn map_section(position: Position) -> Section {
    section(
        "Map",
        MapService::ALL
            .into_iter()
            .map(|service| link(service.name(), service.url(position)))
            .collect(),
    )
}

fn accuracy(metres: f64) -> String {
    if metres < 1000.0 {
        format!("{metres:.0} m")
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}
