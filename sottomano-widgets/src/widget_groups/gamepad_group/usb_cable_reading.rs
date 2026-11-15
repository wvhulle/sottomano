use sottomano_toml_config::severity::Severity;

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
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

pub struct UsbCableReading {
    pub role: UsbCableRole,
}

impl UsbCableReading {
    pub fn reading_row(self) -> ReadingRow<'static> {
        let (label, state) = match self.role {
            UsbCableRole::InputAndCharging => (ReadoutLabel::Unlabeled, ReadingDisplayState::Idle),
            UsbCableRole::ChargingOnly => (
                ReadoutLabel::Labeled("charging only".to_owned()),
                ReadingDisplayState::Warning,
            ),
        };
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "USB cable",
                Phosphor::Usb(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
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
