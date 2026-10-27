use chrono::Locale;
use sottomano_sources::wallpaper_place_article::{Position, Wallpaper};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
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
                KeyValue::new("Latitude", Quantity::angle(position.latitude).decimals(5)),
                KeyValue::new("Longitude", Quantity::angle(position.longitude).decimals(5)),
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
