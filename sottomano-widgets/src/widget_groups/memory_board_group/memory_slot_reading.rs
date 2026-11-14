use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::{
    host_machine_identity::{
        hostname1_os_identity::MachineInfo, udev_hardware_identity::MemorySlotModule,
    },
    non_empty_text::non_empty,
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::{DisplayUnit, Quantity},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct MemorySlotReading {
    pub slot_module: MemorySlotModule,
    pub machine: MachineInfo,
}

impl MemorySlotReading {
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
            Section::dictionary(
                [
                    machine
                        .hardware_vendor
                        .clone()
                        .map(|hardware_vendor| LabeledValue::new("Vendor", hardware_vendor)),
                    machine
                        .hardware_model
                        .clone()
                        .map(|hardware_model| LabeledValue::new("Model", hardware_model)),
                    firmware
                        .map(|firmware_version| LabeledValue::new("Firmware", firmware_version)),
                    firmware_date
                        .map(|firmware_date| LabeledValue::new("Firmware date", firmware_date)),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Board")
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
        Section::dictionary(identity.chain(speed).chain([LabeledValue::new(
            "Module size",
            Quantity::size(module.size),
        )]))
        .titled(slot_module.locator.clone())
    }
}

impl GroupItemReading for MemorySlotReading {
    fn name(&self) -> Cow<'static, str> {
        self.slot_module.locator.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(Quantity::size(self.slot_module.module.size).localized_text(locale))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(
            [Section::from(&self.slot_module)]
                .into_iter()
                .chain(self.board_section()),
        )
    }
}
