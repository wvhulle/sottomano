use chrono::Locale;
use sottomano_desktop_sources::{
    host_machine_identity::{
        hostname1_os_identity::MachineInfo, udev_hardware_identity::MemorySlotModule,
    },
    non_empty_text::non_empty,
};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Clone)]
pub struct MemorySlotIndicator {
    pub slot_module: MemorySlotModule,
    pub machine: MachineInfo,
}

impl MemorySlotIndicator {
    fn board_section(&self) -> Option<Section> {
        let machine = &self.machine;
        (machine.hardware_vendor.is_some() || machine.hardware_model.is_some()).then(|| {
            let firmware = non_empty(
                [
                    machine.firmware_vendor.as_deref(),
                    machine.firmware_version.as_deref(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
            );
            let firmware_date = machine.firmware_date.map(|at| ValueCell::Time {
                at,
                format: LocalizedTimeFormat::Date,
            });
            Section::new(
                "Board",
                dictionary_table(
                    [
                        machine
                            .hardware_vendor
                            .clone()
                            .map(|hardware_vendor| LabeledValue::new("Vendor", hardware_vendor)),
                        machine
                            .hardware_model
                            .clone()
                            .map(|hardware_model| LabeledValue::new("Model", hardware_model)),
                        firmware.map(|firmware_version| {
                            LabeledValue::new("Firmware", firmware_version)
                        }),
                        firmware_date
                            .map(|firmware_date| LabeledValue::new("Firmware date", firmware_date)),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            )
        })
    }
}

impl From<&MemorySlotModule> for Section {
    fn from(slot_module: &MemorySlotModule) -> Self {
        let module = &slot_module.module;
        let speed = module.speed_mts.map(|speed| {
            LabeledValue::new(
                "Speed",
                Quantity::measured(f64::from(speed), DisplayUnit::MegatransfersPerSecond, 0),
            )
        });
        let identity = [
            ("Manufacturer", &module.manufacturer),
            ("Part number", &module.part_number),
            ("Type", &module.kind),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some(LabeledValue::new(key, value.as_deref()?)));
        Section::new(
            "Module",
            dictionary_table(identity.chain(speed).chain([LabeledValue::new(
                "Module size",
                Quantity::size(module.size),
            )])),
        )
    }
}

impl MemorySlotIndicator {
    pub fn indicator<'indicator>(self, locale: Locale) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            self.slot_module.locator.clone(),
            Phosphor::Memory(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                Quantity::size(self.slot_module.module.size).localized_text(locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        [Section::from(&self.slot_module)]
            .into_iter()
            .chain(self.board_section())
            .collect::<Vec<_>>()
    }
}
