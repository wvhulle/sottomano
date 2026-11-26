use sottomano_desktop_sources::{
    gamepad_controller_input::{
        evdev_gamepad_devices::{GamepadTransport, InputRoute, PhysicalGamepadLink},
        steam_input_facts::{RadioReading, SteamInputFacts},
    },
    steam_client_files::steam_log_lines::controller_log_lines::RadioSignalSample,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    widgets::value_cells::LabeledValue,
};

const STRONG_SIGNAL_DBM: i16 = -60;
const MEDIUM_SIGNAL_DBM: i16 = -75;

pub struct SteamRadioIndicator {
    pub signal: RadioSignalSample,
}

impl SteamRadioIndicator {
    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Radio link to the Puck",
                dictionary_table([
                    LabeledValue::new("Signal", format!("{} dBm", self.signal.rssi_dbm)),
                    LabeledValue::new("Channel", self.signal.channel.to_string()),
                ]),
            ),
            Section::new(
                "Measured by Steam",
                SectionWidget::paragraph(
                    "Read from the link reports in Steam's controller log.".to_owned(),
                ),
            ),
        ]
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let glyph = if self.signal.rssi_dbm >= STRONG_SIGNAL_DBM {
            Phosphor::CellSignalHigh(Weight::Regular)
        } else if self.signal.rssi_dbm >= MEDIUM_SIGNAL_DBM {
            Phosphor::CellSignalMedium(Weight::Regular)
        } else {
            Phosphor::CellSignalLow(Weight::Regular)
        };
        GroupIndicator::glyph_only("Controller radio", glyph.into(), IndicatorState::Idle)
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Labeled(format!("{} dBm", self.signal.rssi_dbm)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }
}

impl SteamRadioIndicator {
    pub(super) fn radio_rows<'view>(link: &PhysicalGamepadLink) -> Vec<GroupIndicator<'view>> {
        if matches!(link.transport, GamepadTransport::Bluetooth { .. }) {
            return Vec::new();
        }
        let InputRoute::SteamInput(SteamInputFacts {
            radio: RadioReading::Measured(signal),
            ..
        }) = link.input_route
        else {
            return Vec::new();
        };
        vec![SteamRadioIndicator { signal }.indicator()]
    }
}
