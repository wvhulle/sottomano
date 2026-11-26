use sottomano_desktop_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{InputRoute, PhysicalGamepadLink},
    steam_input_facts::{FirmwareStatus, SteamInputFacts},
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

impl SteamFirmwareIndicator {
    pub(super) fn firmware_rows<'view>(link: &PhysicalGamepadLink) -> Vec<GroupIndicator<'view>> {
        let InputRoute::SteamInput(facts) = link.input_route else {
            return Vec::new();
        };
        if !SteamFirmwareIndicator::has_pending_update(&facts) {
            return Vec::new();
        }
        vec![SteamFirmwareIndicator { facts }.indicator()]
    }
}
