use chrono::Locale;
use sottomano_sources::{
    host_machine_identity::hostname1_os_identity::MachineInfo, non_empty_text::non_empty,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, Section, TooltipBody, entry},
        vendor_logo::{PciVendorId, VendorLogo},
    },
};

impl BarWidget for MachineInfo {
    fn visible_in_bar(&self) -> bool {
        self.hardware_vendor.is_some() || self.hardware_model.is_some()
    }

    fn name(&self) -> &'static str {
        "Board"
    }

    fn icon(&self) -> Glyph {
        let board_logo = self
            .board_pci_vendor
            .and_then(|vendor| PciVendorId(vendor).vendor_logo());
        let coreboot_logo =
            (self.firmware_vendor.as_deref() == Some("coreboot")).then_some(SimpleIcon::Coreboot);
        board_logo
            .or(coreboot_logo)
            .map_or_else(|| Phosphor::Circuitry(Weight::Regular).into(), Glyph::from)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.hardware_model.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
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
        let firmware_date = self.firmware_date.map(|at| Cell::Time {
            at,
            format: LocalizedTimeFormat::Date,
        });
        TooltipBody::sections([Section::dictionary(
            [
                entry("Vendor", self.hardware_vendor.clone()),
                entry("Model", self.hardware_model.clone()),
                entry("Firmware", firmware),
                entry("Firmware date", firmware_date),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Board")])
    }
}
