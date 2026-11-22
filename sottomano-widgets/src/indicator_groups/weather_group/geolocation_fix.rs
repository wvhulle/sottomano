use iced::alignment::Horizontal;
use situo_client::{
    BeaconDbEstimate, CellRadio, DecidingSource, FixOrigin, GeoCoordinates, NearestPlace,
    RadioSource, ServingCell, SourceLookup, SourceLookupOutcome, UpdateTrigger,
};
use sottomano_desktop_sources::{SnapshotSource, weather::situo_location_fix::Location};
use uom::si::{angle::radian, f64::Length, length::kilometer};

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::{DisplayUnit, Quantity},
        value_cells::{LabeledValue, ValueCell},
    },
};

const OPENSTREETMAP_ZOOM: u8 = 15;
const MEAN_EARTH_RADIUS_KILOMETRES: f64 = 6371.0;

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

pub(crate) const LOCATION_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::MapPin(Weight::Regular));

pub fn location_indicator(location: &Location) -> GroupIndicator<'_> {
    let glyph = match &location.geo.origin {
        FixOrigin::Manual => Phosphor::MapPinLine(Weight::Regular).into(),
        FixOrigin::Pinned => Phosphor::PushPin(Weight::Regular).into(),
        FixOrigin::Estimated(estimate) => deciding_source_glyph(estimate.decided_by).into(),
    };
    let label = match &location.geo.place {
        Some(place) if !place.name.is_empty() => ReadoutLabel::Labeled(place.name.clone()),
        Some(_) | None => ReadoutLabel::Unlabeled,
    };
    GroupIndicator::glyph_only(Location::NAME, glyph, IndicatorState::Idle)
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| location_hover_card(location)))
}

pub fn location_hover_card(location: &Location) -> Vec<Section> {
    let estimate = match &location.geo.origin {
        FixOrigin::Manual | FixOrigin::Pinned => None,
        FixOrigin::Estimated(estimate) => Some(estimate),
    };
    let position = location.geo.position;
    let position_section = Section::new(
        "Position",
        dictionary_table([
            LabeledValue::new("Latitude", Quantity::angle(position.latitude).decimals(4)),
            LabeledValue::new("Longitude", Quantity::angle(position.longitude).decimals(4)),
            LabeledValue::new("Source", origin_label(&location.geo.origin)),
        ]),
    );
    vec![
        place_section(location.geo.place.as_ref()),
        position_section,
        estimate_section(estimate),
        source_lookups_section(estimate, position),
        observations_section(estimate),
        held_back_access_points_section(estimate),
        serving_cells_section(estimate),
        updates_section(location),
        map_section(position),
    ]
}

const fn origin_label(origin: &FixOrigin) -> &'static str {
    match origin {
        FixOrigin::Manual => "Configured",
        FixOrigin::Pinned => "Pinned",
        FixOrigin::Estimated(_) => "BeaconDB",
    }
}

const fn deciding_source_glyph(deciding_source: DecidingSource) -> Phosphor {
    match deciding_source {
        DecidingSource::Radio(RadioSource::Wifi) => Phosphor::WifiHigh(Weight::Regular),
        DecidingSource::Radio(RadioSource::Cell) | DecidingSource::CellAreaFallback => {
            Phosphor::CellTower(Weight::Regular)
        }
        DecidingSource::IpAddressFallback => Phosphor::GlobeSimple(Weight::Regular),
        DecidingSource::Undetermined => Phosphor::MapPin(Weight::Regular),
    }
}

const fn deciding_source_label(deciding_source: DecidingSource) -> &'static str {
    match deciding_source {
        DecidingSource::Radio(RadioSource::Wifi) => "Wi-Fi access points",
        DecidingSource::Radio(RadioSource::Cell) => "Serving cell",
        DecidingSource::CellAreaFallback => "Cell location area",
        DecidingSource::IpAddressFallback => "IP address",
        DecidingSource::Undetermined => "Undetermined",
    }
}

const fn radio_source_label(radio_source: RadioSource) -> &'static str {
    match radio_source {
        RadioSource::Wifi => "Wi-Fi",
        RadioSource::Cell => "Cell",
    }
}

const fn cell_radio_label(cell_radio: CellRadio) -> &'static str {
    match cell_radio {
        CellRadio::Gsm => "GSM",
        CellRadio::Wcdma => "WCDMA",
        CellRadio::Lte => "LTE",
        CellRadio::Nr => "NR",
    }
}

fn great_circle_distance(from: GeoCoordinates, to: GeoCoordinates) -> Length {
    let from_latitude = from.latitude.get::<radian>();
    let to_latitude = to.latitude.get::<radian>();
    let half_latitude_change = (to_latitude - from_latitude) / 2.0;
    let half_longitude_change = (to.longitude - from.longitude).get::<radian>() / 2.0;
    let haversine = half_latitude_change.sin().powi(2)
        + from_latitude.cos() * to_latitude.cos() * half_longitude_change.sin().powi(2);
    let central_angle_radians = 2.0 * haversine.sqrt().min(1.0).asin();
    Length::new::<kilometer>(MEAN_EARTH_RADIUS_KILOMETRES * central_angle_radians)
}

fn source_lookup_record(source_lookup: &SourceLookup, published: GeoCoordinates) -> TableRecord<3> {
    let source = radio_source_label(source_lookup.source);
    match source_lookup.outcome {
        SourceLookupOutcome::Located(position_estimate) => TableRecord::with_cells(
            source,
            [
                "located".into(),
                Quantity::distance(position_estimate.accuracy).into(),
                Quantity::distance(great_circle_distance(published, position_estimate.position))
                    .into(),
            ],
        ),
        SourceLookupOutcome::LocatedByCellArea(position_estimate) => TableRecord::with_cells(
            source,
            [
                "cell area".into(),
                Quantity::distance(position_estimate.accuracy).into(),
                Quantity::distance(great_circle_distance(published, position_estimate.position))
                    .into(),
            ],
        ),
        SourceLookupOutcome::NotInDatabase => TableRecord::with_cells(
            source,
            ["not in BeaconDB".into(), ValueCell::Blank, ValueCell::Blank],
        ),
        SourceLookupOutcome::Failed => TableRecord::with_cells(
            source,
            ["lookup failed".into(), ValueCell::Blank, ValueCell::Blank],
        ),
    }
}

fn source_lookups_section(
    estimate: Option<&BeaconDbEstimate>,
    published: GeoCoordinates,
) -> Section {
    let records: Vec<TableRecord<3>> = estimate
        .into_iter()
        .flat_map(|estimate| estimate.source_lookups.iter())
        .map(|source_lookup| source_lookup_record(source_lookup, published))
        .collect();
    Section::new(
        SectionTitle::captioned(
            "Source lookups",
            "What each radio source looks up to on its own at BeaconDB.",
        ),
        headed_table(
            [
                ColumnHeading::titled("Outcome").align_x(Horizontal::Left),
                ColumnHeading::titled("Accuracy"),
                ColumnHeading::titled("Off by"),
            ],
            records,
        ),
    )
}

fn place_section(place: Option<&NearestPlace>) -> Section {
    Section::new(
        "Place",
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

fn estimate_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    Section::new(
        SectionTitle::captioned("Estimate", "How BeaconDB placed you."),
        dictionary_table(estimate.into_iter().flat_map(|estimate| {
            [
                LabeledValue::new("Decided by", deciding_source_label(estimate.decided_by)),
                LabeledValue::new("Accuracy", Quantity::distance(estimate.accuracy)),
            ]
        })),
    )
}

fn observations_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    Section::new(
        SectionTitle::captioned(
            "Observations",
            "What the Wi-Fi scan and the modems reported, and what was sent to BeaconDB.",
        ),
        dictionary_table(estimate.into_iter().flat_map(|estimate| {
            let observations = estimate.observations;
            [
                LabeledValue::new(
                    "Access points seen",
                    Quantity::count(observations.access_points_seen),
                ),
                LabeledValue::new(
                    "Access points sent",
                    Quantity::count(observations.access_points_submitted),
                ),
                LabeledValue::new("Cells seen", Quantity::count(observations.cells_seen)),
                LabeledValue::new("Cells sent", Quantity::count(observations.cells_submitted)),
            ]
        })),
    )
}

fn held_back_access_points_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    Section::new(
        SectionTitle::captioned(
            "Held-back access points",
            "Access points situo did not send, by reason.",
        ),
        dictionary_table(estimate.into_iter().flat_map(|estimate| {
            let exclusions = estimate.observations.access_point_exclusions;
            [
                LabeledValue::new("Phone hotspot", Quantity::count(exclusions.mobile_hotspot)),
                LabeledValue::new("Hidden", Quantity::count(exclusions.hidden)),
                LabeledValue::new("Opted out", Quantity::count(exclusions.opted_out)),
                LabeledValue::new(
                    "Below the two-AP minimum",
                    Quantity::count(exclusions.below_lookup_minimum),
                ),
            ]
        })),
    )
}

fn serving_cell_record(serving_cell: &ServingCell) -> TableRecord<4> {
    TableRecord::with_cells(
        cell_radio_label(serving_cell.radio),
        [
            serving_cell.mobile_country_code.to_string().into(),
            format!("{:02}", serving_cell.mobile_network_code).into(),
            format!("{:#x}", serving_cell.location_area_code).into(),
            format!("{:#x}", serving_cell.cell_id).into(),
        ],
    )
}

fn serving_cells_section(estimate: Option<&BeaconDbEstimate>) -> Section {
    let records: Vec<TableRecord<4>> = estimate
        .into_iter()
        .flat_map(|estimate| estimate.serving_cells.iter())
        .map(serving_cell_record)
        .collect();
    Section::new(
        SectionTitle::captioned("Serving cells", "Cells sent to BeaconDB."),
        headed_table(
            [
                ColumnHeading::titled("MCC"),
                ColumnHeading::titled("MNC"),
                ColumnHeading::titled("Area"),
                ColumnHeading::titled("Cell ID"),
            ],
            records,
        ),
    )
}

const fn update_trigger_label(update_trigger: UpdateTrigger) -> &'static str {
    match update_trigger {
        UpdateTrigger::ServingCellChange => "Serving cell change",
        UpdateTrigger::WifiNeighbourhoodChange => "Wi-Fi neighbourhood change",
        UpdateTrigger::NetworkReconnect => "Network reconnect",
        UpdateTrigger::ModemHotplug => "Modem added or removed",
    }
}

const ALL_UPDATE_TRIGGERS: [UpdateTrigger; 4] = [
    UpdateTrigger::ServingCellChange,
    UpdateTrigger::WifiNeighbourhoodChange,
    UpdateTrigger::NetworkReconnect,
    UpdateTrigger::ModemHotplug,
];

fn updates_section(location: &Location) -> Section {
    let schedule_rows = [
        LabeledValue::new(
            "Fallback interval",
            ValueCell::duration(location.fallback_interval()),
        ),
        LabeledValue::new(
            "Cell check interval",
            ValueCell::duration(location.cell_check_interval()),
        ),
    ];
    let trigger_rows = ALL_UPDATE_TRIGGERS.map(|update_trigger| {
        let watch_state = if location.update_schedule.triggers.contains(&update_trigger) {
            "watched"
        } else {
            "not watched"
        };
        LabeledValue::new(update_trigger_label(update_trigger), watch_state)
    });
    Section::new(
        SectionTitle::captioned(
            "Updates",
            "When situo looks up again; the fallback interval applies when nothing else triggers.",
        ),
        dictionary_table(schedule_rows.into_iter().chain(trigger_rows)),
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

#[cfg(test)]
mod tests {
    use uom::si::length::meter;

    use super::*;

    #[test]
    fn great_circle_distance_between_ghent_and_brussels() {
        let ghent = GeoCoordinates::from_degrees(51.0543, 3.7174).unwrap();
        let brussels = GeoCoordinates::from_degrees(50.8503, 4.3517).unwrap();
        let kilometres = great_circle_distance(ghent, brussels).get::<kilometer>();
        assert!((48.0..50.0).contains(&kilometres), "{kilometres}");
        assert!(great_circle_distance(ghent, ghent).get::<meter>().abs() < 1e-6);
    }
}
