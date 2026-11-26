use situo_client::{FixOrigin, GeoCoordinates, NearestPlace};
use sottomano_desktop_sources::weather::situo_location_fix::Location;
use uom::si::length::kilometer;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::agenda_menu::location_group::estimate_face::deciding_source_glyph,
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        value_cells::{LabeledValue, ValueCell},
    },
};

const OPENSTREETMAP_ZOOM: u8 = 15;

pub struct PlaceFace<'view> {
    pub location: &'view Location,
}

impl<'view> PlaceFace<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let glyph = match &self.location.geo.origin {
            FixOrigin::Manual => Phosphor::MapPinLine(Weight::Regular),
            FixOrigin::Pinned => Phosphor::PushPin(Weight::Regular),
            FixOrigin::Estimated(estimate) => deciding_source_glyph(estimate.decided_by),
        };
        let label = match &self.location.geo.place {
            Some(place) if !place.name.is_empty() => ReadoutLabel::Labeled(place.name.clone()),
            Some(_) | None => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only("Place", glyph.into(), IndicatorState::Idle).with_label(label)
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let position = self.location.geo.position;
        vec![
            place_section(self.location.geo.place.as_ref()),
            Section::new(
                "Position",
                dictionary_table([
                    LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(4)),
                    LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(4)),
                    LabeledValue::new("Source", origin_label(&self.location.geo.origin)),
                ]),
            ),
            map_section(position),
        ]
    }
}

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

    fn glyph(self) -> IconGlyph {
        match self {
            Self::GoogleMaps => SimpleIcon::GoogleMaps.into(),
            Self::OpenStreetMap => SimpleIcon::OpenStreetMap.into(),
        }
    }

    fn url(self, position: GeoCoordinates) -> String {
        let latitude = position.latitude_degrees();
        let longitude = position.longitude_degrees();
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

const fn origin_label(origin: &FixOrigin) -> &'static str {
    match origin {
        FixOrigin::Manual => "Configured",
        FixOrigin::Pinned => "Pinned",
        FixOrigin::Estimated(_) => "BeaconDB",
    }
}

fn place_section(place: Option<&NearestPlace>) -> Section {
    Section::new(
        "Nearest place",
        dictionary_table(place.into_iter().flat_map(|place| {
            [
                LabeledValue::new("Name", place.name.as_str()),
                LabeledValue::new("Country", place.country.as_str()),
                LabeledValue::new(
                    "Distance",
                    Quantity::measured(
                        place.distance.get::<kilometer>(),
                        DisplayUnit::Kilometer,
                        2,
                    ),
                ),
            ]
        })),
    )
}

fn map_section(position: GeoCoordinates) -> Section {
    let latitude = position.latitude_degrees();
    let longitude = position.longitude_degrees();
    Section::new(
        "Map",
        dictionary_table(MapService::ALL.map(|service| {
            LabeledValue::new(
                service.name(),
                ValueCell::BrandedLink {
                    label: format!("{latitude:.5}, {longitude:.5}"),
                    url: service.url(position),
                    glyph: service.glyph(),
                },
            )
        })),
    )
}
