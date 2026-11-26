use sottomano_desktop_sources::{
    gamepad_controller_input::steam_input_facts::{FirmwareStatus, SteamInputFacts},
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

pub struct SteamFirmwareIndicator {
    pub facts: SteamInputFacts,
}

impl SteamFirmwareIndicator {
    #[must_use]
    pub fn has_pending_update(facts: &SteamInputFacts) -> bool {
        facts.controller_firmware == FirmwareStatus::UpdatePending
            || facts.dongle_firmware == FirmwareStatus::UpdatePending
    }

    fn pending_hardware(&self) -> Vec<LabeledValue> {
        [
            (self.facts.controller_firmware, "Controller"),
            (self.facts.dongle_firmware, "Puck dongle"),
        ]
        .into_iter()
        .filter(|(status, _)| *status == FirmwareStatus::UpdatePending)
        .map(|(_, hardware)| LabeledValue::new(hardware, "update available"))
        .collect()
    }

    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Firmware update",
            Phosphor::ArrowCircleUp(Weight::Regular).into(),
            IndicatorState::Warning,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("firmware update".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| {
            vec![
                Section::new("Firmware", dictionary_table(self.pending_hardware())),
                Section::new(
                    "Reported by Steam",
                    SectionWidget::paragraph(
                        "Steam's hardware updater found newer firmware during its last check."
                            .to_owned(),
                    ),
                ),
            ]
        }))
    }
}
