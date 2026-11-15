use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::wallpaper_place_article::{Position, Wallpaper};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{Section, dictionary_table},
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
    fn name(&self) -> Cow<'static, str> {
        "Wallpaper place".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::MapPin(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.name.clone())
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let coordinates = self.position.into_iter().flat_map(|position| {
            [
                LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(5)),
                LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(5)),
            ]
        });
        vec![Section::new(
            "Location",
            dictionary_table(
                [LabeledValue::new("Where", self.name.as_str())]
                    .into_iter()
                    .chain(coordinates),
            ),
        )]
    }
}

impl Place {
    pub(super) fn wallpaper_place(wallpaper: &Wallpaper) -> Option<Self> {
        Some(Self {
            name: wallpaper.place.clone()?,
            position: wallpaper.position,
        })
    }
}
