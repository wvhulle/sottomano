use sottomano_desktop_sources::battery_power_profiles::upower_battery_state::{
    Battery, ChargeStatus,
};
use uom::si::{f64::Ratio, ratio::percent};

use super::{PowerGroup, battery_refresh};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::hardware_menu::power_group::charger_connection::ChargerConnection,
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub const CHARGE_INDICATOR_NAME: &str = "Charge";

pub(super) const BATTERY_CHARGE_GLYPH: Phosphor = Phosphor::BatteryFull(Weight::Regular);

fn capacity_state(capacity: Ratio) -> IndicatorState {
    if capacity <= Ratio::new::<percent>(15.0) {
        IndicatorState::Critical
    } else if capacity <= Ratio::new::<percent>(30.0) {
        IndicatorState::Warning
    } else {
        IndicatorState::Idle
    }
}

pub fn battery_indicator(battery: &Battery) -> GroupIndicator<'_> {
    let connection = ChargerConnection::from(battery);
    GroupIndicator::glyph_only(
        CHARGE_INDICATOR_NAME,
        connection.battery_glyph(battery.capacity),
        connection.indicator_state(capacity_state(battery.capacity)),
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Gauged {
            fill: battery.capacity,
            live: battery.capacity,
        },
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| battery_hover_card(battery)))
}

pub fn battery_hover_card(battery: &Battery) -> Vec<Section> {
    vec![Section::new(
        "Battery level",
        dictionary_table(
            [
                Some(LabeledValue::new(
                    "Level",
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

impl PowerGroup {
    pub(super) fn battery_charge_listing(&self) -> IndicatorListing<'_> {
        self.battery.indicator_listing(
            |held_snapshot| {
                self.battery_charge_indicator(battery_indicator(held_snapshot))
                    .when_shown()
            },
            BATTERY_CHARGE_GLYPH.into(),
        )
    }

    fn battery_charge_indicator<'view>(
        &'view self,
        charge_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        charge_indicator
            .with_hover_card(Box::new(move |_| {
                let mut sections = self.battery.sections_described_by(battery_hover_card);
                sections.extend(
                    self.battery
                        .sections_described_by(|battery| vec![Section::from(battery)]),
                );
                sections
            }))
            .refreshed_by(battery_refresh())
    }
}
