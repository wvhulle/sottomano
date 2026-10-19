use sottomano_sources::{
    host_machine_identity::hostname1_os_identity::MachineInfo, non_empty_text::non_empty,
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, Section, TooltipBody, entry},
};

impl BarWidget for MachineInfo {
    fn visible_in_bar(&self) -> bool {
        self.hardware_vendor.is_some() || self.hardware_model.is_some()
    }

    fn name(&self) -> &'static str {
        "Board"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Circuitry(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.hardware_model.clone()
    }

    fn tooltip(&self) -> TooltipBody {
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
