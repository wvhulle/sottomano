use chrono::Locale;
use sottomano_sources::wallpaper_place_article::{Position, Wallpaper};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    name: String,
    position: Option<Position>,
}

impl GroupItemReading for Place {
    fn name(&self) -> &'static str {
        "Wallpaper place"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.name.clone())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let coordinates = self.position.into_iter().flat_map(|position| {
            [
                LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(5)),
                LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(5)),
            ]
        });
        HoverCardBody::dictionary(
            [LabeledValue::new("Where", self.name.as_str())]
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
