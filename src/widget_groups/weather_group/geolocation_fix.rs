use situo_client::Source as FixSource;

use crate::{
    data::source::weather::Location,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Location {
    fn icon(&self) -> Glyph {
        if self.geo.source == FixSource::Manual {
            Solid::LocationPinLock.into()
        } else {
            Solid::LocationDot.into()
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let dash = |value: &str| {
            if value.is_empty() {
                "—".to_string()
            } else {
                value.to_string()
            }
        };
        TooltipBody::rows(vec![
            row("Place", dash(&self.geo.place_name)),
            row("Country", dash(&self.geo.place_country)),
            row("Distance", distance(self.geo.place_distance_km)),
            row("Latitude", format!("{:.4}", self.geo.latitude)),
            row("Longitude", format!("{:.4}", self.geo.longitude)),
            row("Accuracy", accuracy(self.geo.accuracy)),
            row("Source", dash(self.geo.source.as_str())),
            row("Method", dash(&self.geo.description)),
        ])
    }
}

fn distance(km: f64) -> String {
    if km.is_nan() {
        "—".to_string()
    } else {
        format!("{km:.2} km")
    }
}

fn accuracy(metres: f64) -> String {
    if metres.is_nan() {
        "—".to_string()
    } else if metres < 1000.0 {
        format!("{metres:.0} m")
    } else {
        format!("{:.1} km", metres / 1000.0)
    }
}
