use chrono::Locale;
use sottomano_sources::wallpaper_place_article::{Position, Wallpaper};
use uom::{fmt::DisplayStyle::Abbreviation, si::angle::degree};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    position: Option<Position>,
}

impl BarWidget for Place {
    fn name(&self) -> &'static str {
        "Wallpaper place"
    }

    fn icon(&self) -> Glyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.name.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let coordinates = self.position.into_iter().flat_map(|position| {
            [
                KeyValue::new(
                    "Latitude",
                    format!(
                        "{:.5}",
                        position.latitude.into_format_args(degree, Abbreviation)
                    ),
                ),
                KeyValue::new(
                    "Longitude",
                    format!(
                        "{:.5}",
                        position.longitude.into_format_args(degree, Abbreviation)
                    ),
                ),
            ]
        });
        TooltipBody::dictionary(
            [KeyValue::new("Where", self.name.as_str())]
                .into_iter()
                .chain(coordinates),
        )
    }
}

impl Place {
    pub(super) fn of(wallpaper: &Wallpaper) -> Option<Self> {
        Some(Self {
            name: wallpaper.place.clone()?,
            position: wallpaper.position,
        })
    }
}
