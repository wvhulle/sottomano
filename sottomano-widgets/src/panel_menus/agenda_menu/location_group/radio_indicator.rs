use situo_client::{BeaconDbEstimate, CellRadio, FixOrigin, ServingCell};
use sottomano_desktop_sources::weather::situo_location_fix::Location;

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
        value_cells::LabeledValue,
    },
};

pub struct RadioIndicator<'view> {
    pub estimate: &'view BeaconDbEstimate,
}

impl<'view> RadioIndicator<'view> {
    pub fn indicator(self) -> GroupIndicator<'view> {
        let observations = self.estimate.observations;
        GroupIndicator::glyph_only(
            "Radio",
            Phosphor::Broadcast(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_label(ReadoutLabel::Labeled(format!(
            "{}/{} APs",
            observations.access_points_submitted, observations.access_points_seen
        )))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![
            observations_section(self.estimate),
            held_back_access_points_section(self.estimate),
            serving_cells_section(self.estimate),
        ]
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

fn observations_section(estimate: &BeaconDbEstimate) -> Section {
    Section::new(
        SectionTitle::captioned(
            "Observations",
            "What the Wi-Fi scan and the modems reported, and what was sent to BeaconDB.",
        ),
        dictionary_table({
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
        }),
    )
}

fn held_back_access_points_section(estimate: &BeaconDbEstimate) -> Section {
    Section::new(
        SectionTitle::captioned(
            "Held-back access points",
            "Access points situo did not send, by reason.",
        ),
        dictionary_table({
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
        }),
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

fn serving_cells_section(estimate: &BeaconDbEstimate) -> Section {
    let records: Vec<TableRecord<4>> = estimate
        .serving_cells
        .iter()
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

impl LocationGroup {
    pub(super) fn radio_rows<'view>(
        &'view self,
        location: &'view Location,
    ) -> Vec<GroupIndicator<'view>> {
        let FixOrigin::Estimated(estimate) = &location.geo.origin else {
            return Vec::new();
        };
        vec![
            RadioIndicator { estimate }
                .indicator()
                .with_hover_card(Box::new(move |_| {
                    self.location
                        .hover_card_with(RadioIndicator { estimate }.hover_card())
                })),
        ]
    }
}
