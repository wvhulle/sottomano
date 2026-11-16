use sottomano_toml_config::severity::Severity;

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionWidget, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbCableRole {
    InputAndCharging,
    ChargingOnly,
}

pub struct UsbCableIndicator {
    pub role: UsbCableRole,
}

impl UsbCableIndicator {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let (label, state) = match self.role {
            UsbCableRole::InputAndCharging => (ReadoutLabel::Unlabeled, IndicatorState::Idle),
            UsbCableRole::ChargingOnly => (
                ReadoutLabel::Labeled("charging only".to_owned()),
                IndicatorState::Warning,
            ),
        };
        GroupIndicator::glyph_only("USB cable", Phosphor::Usb(Weight::Regular).into(), state)
            .with_readout(IndicatorReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        match self.role {
            UsbCableRole::InputAndCharging => {
                vec![Section::new(
                    "Connection",
                    dictionary_table([LabeledValue::new("Carries", "input and charging")]),
                )]
            }
            UsbCableRole::ChargingOnly => vec![
                Section::new(
                    "Connection",
                    dictionary_table([
                        LabeledValue::new("Carries", "charging only").level(Severity::Warning)
                    ]),
                ),
                Section::new(
                    "Input still goes over Bluetooth",
                    SectionWidget::paragraph(
                        "The controller was already connected over Bluetooth when the cable was \
                         plugged in, so the kernel ignored the cable. Press to disconnect \
                         Bluetooth, then plug the cable in again."
                            .to_owned(),
                    ),
                ),
            ],
        }
    }
}
