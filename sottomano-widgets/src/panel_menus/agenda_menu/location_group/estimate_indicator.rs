use iced::alignment::Horizontal;
use situo_client::{
    BeaconDbEstimate, DecidingSource, FixOrigin, GeoCoordinates, RadioSource, SourceLookup,
    SourceLookupOutcome,
};
use sottomano_desktop_sources::weather::situo_location_fix::Location;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        angle::radian,
        f64::Length,
        length::{kilometer, meter},
    },
};

use super::LocationGroup;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

const MEAN_EARTH_RADIUS_KILOMETRES: f64 = 6371.0;

pub struct EstimateIndicator<'view> {
    pub estimate: &'view BeaconDbEstimate,
    pub published: GeoCoordinates,
}

impl<'view> EstimateIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let accuracy_label = format!(
            "{:.0}",
            self.estimate.accuracy.into_format_args(meter, Abbreviation)
        );
        GroupIndicator::glyph_only(
            "Estimate",
            Phosphor::Crosshair(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(accuracy_label))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![
            estimate_section(self.estimate),
            source_lookups_section(self.estimate, self.published),
        ]
    }
}

pub const fn deciding_source_glyph(deciding_source: DecidingSource) -> Phosphor {
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

fn source_lookups_section(estimate: &BeaconDbEstimate, published: GeoCoordinates) -> Section {
    let records: Vec<TableRecord<3>> = estimate
        .source_lookups
        .iter()
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

fn estimate_section(estimate: &BeaconDbEstimate) -> Section {
    Section::new(
        SectionTitle::captioned("BeaconDB answer", "How BeaconDB placed you."),
        dictionary_table([
            LabeledValue::new("Decided by", deciding_source_label(estimate.decided_by)),
            LabeledValue::new("Accuracy", Quantity::distance(estimate.accuracy)),
        ]),
    )
}

impl LocationGroup {
    pub(super) fn estimate_rows<'view>(
        &'view self,
        location: &'view Location,
    ) -> Vec<GroupIndicator<'view>> {
        let FixOrigin::Estimated(estimate) = &location.geo.origin else {
            return Vec::new();
        };
        let published = location.geo.position;
        vec![
            EstimateIndicator {
                estimate,
                published,
            }
            .indicator()
            .with_hover_card(Box::new(move |_| {
                self.location.hover_card_with(
                    EstimateIndicator {
                        estimate,
                        published,
                    }
                    .hover_card(),
                )
            })),
        ]
    }
}

#[cfg(test)]
mod tests {
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
