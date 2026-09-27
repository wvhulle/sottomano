use situo_client::{Estimate, Origin, Place, Position};

use crate::{
    data::source::weather::Location,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        tooltip_body::{Row, TooltipBody, row},
    },
};

impl BarWidget for Location {
    fn icon(&self) -> Glyph {
        match self.geo.origin {
            Origin::Manual => Solid::LocationPinLock.into(),
            Origin::Estimated(_) => Solid::LocationDot.into(),
        }
    }

    fn fallback_icon() -> Glyph {
        Solid::LocationDot.into()
    }

    fn label(&self) -> Option<String> {
        self.geo.place.as_ref().map(|place| place.name.clone())
    }

    fn tooltip(&self) -> TooltipBody {
        let place = self.geo.place.iter().flat_map(place_rows);
        let estimate = match &self.geo.origin {
            Origin::Manual => Vec::new(),
            Origin::Estimated(estimate) => estimate_rows(estimate),
        };
        TooltipBody::rows(
            place
                .chain(position_rows(self.geo.position))
                .chain(estimate)
                .chain([row("Source", self.geo.origin.to_string())])
                .collect(),
        )
    }
}

fn place_rows(place: &Place) -> [Row; 3] {
    [
        row("Place", place.name.clone()),
        row("Country", place.country.clone()),
        row("Distance", format!("{:.2} km", place.distance_km)),
    ]
}

fn position_rows(position: Position) -> [Row; 2] {
    [
        row("Latitude", format!("{:.4}", position.latitude)),
        row("Longitude", format!("{:.4}", position.longitude)),
    ]
}

fn estimate_rows(estimate: &Estimate) -> Vec<Row> {
    let observations = estimate.observations;
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
    ]
}

fn accuracy(metres: f64) -> String {
    if metres < 1000.0 {
        format!("{metres:.0} m")
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}
