use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::{
    host_machine_identity::{
        hostname1_os_identity::MachineInfo,
        udev_hardware_identity::{MemoryModule, MemorySlotModule},
    },
    non_empty_text::non_empty,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchingModuleBank {
    pub slot_locators: Vec<String>,
    pub module: MemoryModule,
}

impl MatchingModuleBank {
    fn bank_title(&self) -> String {
        match self.slot_locators.as_slice() {
            [single_locator] => single_locator.clone(),
            _ => self
                .module
                .kind
                .clone()
                .unwrap_or_else(|| "Memory".to_owned()),
        }
    }

    fn total_size(&self) -> ByteSize {
        let module_count =
            u64::try_from(self.slot_locators.len()).expect("a machine has fewer than 2^64 slots");
        ByteSize::b(self.module.size.as_u64() * module_count)
    }
}

pub fn matching_module_banks(slots: &[MemorySlotModule]) -> Vec<MatchingModuleBank> {
    slots.iter().fold(Vec::new(), |mut banks, slot| {
        match banks
            .iter_mut()
            .find(|bank: &&mut MatchingModuleBank| bank.module == slot.module)
        {
            Some(matching_bank) => matching_bank.slot_locators.push(slot.locator.clone()),
            None => banks.push(MatchingModuleBank {
                slot_locators: vec![slot.locator.clone()],
                module: slot.module.clone(),
            }),
        }
        banks
    })
}

#[derive(Clone)]
pub struct MemorySlotIndicator {
    pub matching_module_bank: MatchingModuleBank,
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

impl From<&MatchingModuleBank> for Section {
    fn from(bank: &MatchingModuleBank) -> Self {
        let module = &bank.module;
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
        let module_count = (bank.slot_locators.len() > 1)
            .then(|| LabeledValue::new("Modules", Quantity::count(bank.slot_locators.len())));
        Section::new(
            "Module",
            dictionary_table(
                identity
                    .chain(speed)
                    .chain(module_count)
                    .chain([LabeledValue::new(
                        "Module size",
                        Quantity::size(module.size),
                    )]),
            ),
        )
    }
}

fn slots_section(bank: &MatchingModuleBank) -> Section {
    Section::new(
        "Slots",
        dictionary_table(bank.slot_locators.iter().map(|slot_locator| {
            LabeledValue::new(slot_locator.clone(), Quantity::size(bank.module.size))
        })),
    )
}

impl MemorySlotIndicator {
    pub fn indicator<'indicator>(self, locale: Locale) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            self.matching_module_bank.bank_title(),
            Phosphor::Memory(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                Quantity::size(self.matching_module_bank.total_size()).localized_text(locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        [
            Section::from(&self.matching_module_bank),
            slots_section(&self.matching_module_bank),
        ]
        .into_iter()
        .chain(self.board_section())
        .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use sottomano_desktop_sources::host_machine_identity::udev_hardware_identity::{
        MemoryModule, MemorySlotModule,
    };

    use super::matching_module_banks;

    fn lpddr5_module(size_gibibytes: u64) -> MemoryModule {
        MemoryModule {
            manufacturer: Some("Samsung".to_owned()),
            part_number: None,
            kind: Some("LPDDR5".to_owned()),
            speed_mts: Some(8533),
            size: ByteSize::gib(size_gibibytes),
        }
    }

    fn slot_holding(locator: &str, module: MemoryModule) -> MemorySlotModule {
        MemorySlotModule {
            locator: locator.to_owned(),
            module,
        }
    }

    #[test]
    fn identical_soldered_channels_form_one_bank() {
        let channel_slots: Vec<MemorySlotModule> = ["A", "B", "C", "D"]
            .into_iter()
            .flat_map(|channel| {
                [0, 1].map(|controller| {
                    slot_holding(
                        &format!("Controller{controller}-Channel{channel}"),
                        lpddr5_module(4),
                    )
                })
            })
            .collect();
        let banks = matching_module_banks(&channel_slots);
        assert_eq!(banks.len(), 1);
        assert_eq!(banks[0].slot_locators.len(), 8);
        assert_eq!(banks[0].bank_title(), "LPDDR5");
        assert_eq!(banks[0].total_size(), ByteSize::gib(32));
    }

    #[test]
    fn interleaved_kits_form_banks_in_firmware_order() {
        let mixed_slots = [
            slot_holding("DIMM A1", lpddr5_module(16)),
            slot_holding("DIMM A2", lpddr5_module(8)),
            slot_holding("DIMM B1", lpddr5_module(16)),
            slot_holding("DIMM B2", lpddr5_module(8)),
        ];
        let banks = matching_module_banks(&mixed_slots);
        let bank_locators: Vec<&[String]> = banks
            .iter()
            .map(|bank| bank.slot_locators.as_slice())
            .collect();
        assert_eq!(
            bank_locators,
            [
                ["DIMM A1".to_owned(), "DIMM B1".to_owned()].as_slice(),
                ["DIMM A2".to_owned(), "DIMM B2".to_owned()].as_slice(),
            ]
        );
    }

    #[test]
    fn single_slot_bank_keeps_its_locator() {
        let banks = matching_module_banks(&[slot_holding("DIMM A1", lpddr5_module(16))]);
        assert_eq!(banks[0].bank_title(), "DIMM A1");
    }
}
