use vetrata_sources::{empty_text::non_empty, hostname::MachineInfo};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::{Phosphor, Weight},
    tooltip_body::{Cell, Section, TooltipBody, entry},
};

pub struct BoardFirmwareFace<'a> {
    machine: &'a MachineInfo,
}

impl<'a> BoardFirmwareFace<'a> {
    pub fn identified(machine: &'a MachineInfo) -> Option<Self> {
        (machine.hardware_vendor.is_some() || machine.hardware_model.is_some())
            .then_some(Self { machine })
    }
}

impl BarWidget for BoardFirmwareFace<'_> {
    fn name(&self) -> &'static str {
        "Board"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Circuitry(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.machine.hardware_model.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        let firmware = non_empty(
            [
                self.machine.firmware_vendor.as_deref(),
                self.machine.firmware_version.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
        );
        let firmware_date = self.machine.firmware_date.map(|at| Cell::Time {
            at,
            format: LocalizedTimeFormat::Date,
        });
        TooltipBody::sections([Section::dictionary(
            [
                entry("Vendor", self.machine.hardware_vendor.clone()),
                entry("Model", self.machine.hardware_model.clone()),
                entry("Firmware", firmware),
                entry("Firmware date", firmware_date),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Board")])
    }
}
