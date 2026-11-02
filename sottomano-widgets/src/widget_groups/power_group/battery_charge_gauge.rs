use sottomano_sources::battery_power_profiles::upower_battery_state::Battery;
use uom::si::{f64::Ratio, ratio::percent};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

impl GroupItemReading for Battery {
    fn name(&self) -> &'static str {
        "Battery"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::BatteryCharging(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.capacity <= Ratio::new::<percent>(15.0) {
            WidgetState::Critical
        } else if self.capacity <= Ratio::new::<percent>(30.0) {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.capacity)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary(
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
