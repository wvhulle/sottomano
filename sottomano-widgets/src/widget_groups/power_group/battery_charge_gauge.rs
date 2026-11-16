use sottomano_desktop_sources::{
    SnapshotSource,
    battery_power_profiles::upower_battery_state::{Battery, ChargeStatus},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub fn battery_indicator(battery: &Battery) -> ReadingRow<'_> {
    let state = if battery.capacity <= Ratio::new::<percent>(15.0) {
        IndicatorState::Critical
    } else if battery.capacity <= Ratio::new::<percent>(30.0) {
        IndicatorState::Warning
    } else {
        IndicatorState::Idle
    };
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Gauged {
                fill: battery.capacity,
                live: battery.capacity,
            },
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            Battery::NAME,
            Phosphor::BatteryCharging(Weight::Regular).into(),
            state,
            Box::new(move |_| battery_hover_card(battery)),
        )
    }
}

pub fn battery_hover_card(battery: &Battery) -> Vec<Section> {
    vec![Section::new(
        "Charge",
        dictionary_table(
            [
                Some(LabeledValue::new(
                    "Battery",
                    Quantity::share(battery.capacity),
                )),
                (battery.status != ChargeStatus::Unreported)
                    .then(|| LabeledValue::new("Status", battery.status.as_str())),
                Some(LabeledValue::new(
                    "Remaining",
                    Quantity::energy(battery.energy),
                )),
            ]
            .into_iter()
            .flatten(),
        ),
    )]
}
