use crate::{
    data::source::weather::Wind,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

const BEAUFORT_LOWER_BOUNDS_KM_PER_H: [f64; 12] = [
    1.0, 6.0, 12.0, 20.0, 29.0, 39.0, 50.0, 62.0, 75.0, 89.0, 103.0, 118.0,
];

const COMPASS_POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

impl Wind {
    fn beaufort(&self) -> usize {
        BEAUFORT_LOWER_BOUNDS_KM_PER_H
            .iter()
            .take_while(|&&bound| self.speed >= bound)
            .count()
    }

    fn compass_point(&self) -> &'static str {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sector = (self.direction.rem_euclid(360.0) / 45.0).round() as usize;
        COMPASS_POINTS[sector % COMPASS_POINTS.len()]
    }
}

impl BarWidget for Wind {
    fn icon(&self) -> Glyph {
        Solid::Wind.into()
    }

    fn label(&self) -> Option<String> {
        Some(self.beaufort().to_string())
    }

    fn level(&self) -> Level {
        match self.beaufort() {
            0..=6 => Level::Ok,
            7..=8 => Level::Warning,
            _ => Level::Critical,
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row(
                "Speed",
                format!("{} km/h (Beaufort {})", self.speed, self.beaufort()),
            ),
            row("Gusts", format!("{} km/h", self.gusts)),
            row(
                "From",
                format!("{} ({}°)", self.compass_point(), self.direction),
            ),
        ])
    }
}
