use sottomano_sources::battery_power_profiles::upower_battery_state::Battery;
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for Battery {
    fn name(&self) -> &'static str {
        "Battery"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::BatteryCharging(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.capacity <= Ratio::new::<percent>(15.0) {
            ReadingDisplayState::Critical
        } else if self.capacity <= Ratio::new::<percent>(30.0) {
            ReadingDisplayState::Warning
        } else {
            ReadingDisplayState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.capacity)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            [
                Some(LabeledValue::new("Battery", Quantity::share(self.capacity))),
                self.status
                    .map(|status| LabeledValue::new("Status", status.as_str())),
                Some(LabeledValue::new(
                    "Remaining",
                    Quantity::energy(self.energy),
                )),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
