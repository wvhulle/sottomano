use chrono::Locale;
use sottomano_sources::{
    host_machine_identity::hostname1_os_identity::MachineInfo, non_empty_text::non_empty,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
        vendor_logo::{PciVendorId, VendorLogo},
    },
};

impl GroupItemReading for MachineInfo {
    fn visible_on_panel(&self) -> bool {
        self.hardware_vendor.is_some() || self.hardware_model.is_some()
    }

    fn name(&self) -> &'static str {
        "Board"
    }

    fn icon(&self) -> IconGlyph {
        let board_logo = self
            .board_pci_vendor
            .and_then(|vendor| PciVendorId(vendor).vendor_logo());
        let coreboot_logo =
            (self.firmware_vendor.as_deref() == Some("coreboot")).then_some(SimpleIcon::Coreboot);
        board_logo.or(coreboot_logo).map_or_else(
            || Phosphor::Circuitry(Weight::Regular).into(),
            IconGlyph::from,
        )
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.hardware_model.clone()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let firmware = non_empty(
            [
                self.firmware_vendor.as_deref(),
                self.firmware_version.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
        );
        let firmware_date = self.firmware_date.map(|at| ValueCell::Time {
            at,
            format: LocalizedTimeFormat::Date,
        });
        HoverCardBody::sections([Section::dictionary(
            [
                self.hardware_vendor
                    .clone()
                    .map(|hardware_vendor| LabeledValue::new("Vendor", hardware_vendor)),
                self.hardware_model
                    .clone()
                    .map(|hardware_model| LabeledValue::new("Model", hardware_model)),
                firmware.map(|firmware_version| LabeledValue::new("Firmware", firmware_version)),
                firmware_date
                    .map(|firmware_date| LabeledValue::new("Firmware date", firmware_date)),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Board")])
    }
}
