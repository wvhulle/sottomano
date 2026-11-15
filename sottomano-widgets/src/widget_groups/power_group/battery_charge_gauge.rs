use sottomano_desktop_sources::{
    SnapshotSource,
    battery_power_profiles::upower_battery_state::{Battery, ChargeStatus},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub fn battery_reading_row(battery: &Battery) -> ReadingRow<'_> {
    let state = if battery.capacity <= Ratio::new::<percent>(15.0) {
        ReadingDisplayState::Critical
    } else if battery.capacity <= Ratio::new::<percent>(30.0) {
        ReadingDisplayState::Warning
    } else {
        ReadingDisplayState::Idle
    };
    ReadingRow {
        readout: RowReadout {
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
            Box::new(move |clock| battery_hover_card(battery, clock)),
        )
    }
}

pub fn battery_hover_card(battery: &Battery, _clock: Clock) -> Vec<Section> {
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
