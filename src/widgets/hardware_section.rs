use crate::{
    data::source::hardware::{
        Adapter, CpuModel, CpuSignature, DriveIdentity, MemoryModuleGroup, MemoryModules,
        ModemIdentity, PowerSupplyIdentity,
    },
    widgets::tooltip_body::{Row, Section, link, row, section},
};

pub trait HardwareRows {
    fn hardware_rows(&self) -> Vec<Row>;

    fn hardware_section(&self) -> Section {
        section("Hardware", self.hardware_rows())
    }
}

impl<H: HardwareRows> HardwareRows for &H {
    fn hardware_rows(&self) -> Vec<Row> {
        (*self).hardware_rows()
    }
}

impl<H: HardwareRows> HardwareRows for Option<H> {
    fn hardware_rows(&self) -> Vec<Row> {
        self.as_ref().map(H::hardware_rows).unwrap_or_default()
    }
}

pub fn reported_rows<'a>(
    facts: impl IntoIterator<Item = (&'static str, Option<&'a String>)>,
) -> Vec<Row> {
    facts
        .into_iter()
        .filter_map(|(key, value)| Some(row(key, value?.clone())))
        .collect()
}

impl HardwareRows for CpuModel {
    fn hardware_rows(&self) -> Vec<Row> {
        let mut rows = reported_rows([
            ("Vendor", self.vendor.as_ref()),
            ("Model", self.name.as_ref()),
        ]);
        rows.extend(self.signature.map(CpuSignature::rows).unwrap_or_default());
        rows
    }
}

impl CpuSignature {
    fn rows(self) -> Vec<Row> {
        match self {
            CpuSignature::X86 {
                family,
                model,
                stepping,
            } => vec![
                row("Family", family.to_string()),
                row("Model number", model.to_string()),
                row("Stepping", stepping.to_string()),
            ],
            CpuSignature::Arm { implementer, part } => vec![
                row("Implementer", format!("0x{implementer:02x}")),
                row("Part", format!("0x{part:03x}")),
            ],
        }
    }
}

impl HardwareRows for MemoryModules {
    fn hardware_rows(&self) -> Vec<Row> {
        self.0.iter().flat_map(MemoryModuleGroup::rows).collect()
    }
}

impl MemoryModuleGroup {
    fn rows(&self) -> Vec<Row> {
        let module = &self.module;
        let mut rows = reported_rows([
            ("Manufacturer", module.manufacturer.as_ref()),
            ("Part number", module.part_number.as_ref()),
            ("Type", module.kind.as_ref()),
        ]);
        rows.extend(
            module
                .speed_mts
                .map(|speed| row("Speed", format!("{speed} MT/s"))),
        );
        rows.push(row("Module size", module.size.display().iec().to_string()));
        rows.push(row("Modules", self.count.to_string()));
        rows
    }
}

impl HardwareRows for DriveIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        reported_rows([
            ("Model", self.model.as_ref()),
            ("Serial", self.serial.as_ref()),
            ("Firmware", self.firmware.as_ref()),
        ])
    }
}

impl HardwareRows for PowerSupplyIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        reported_rows([
            ("Manufacturer", self.manufacturer.as_ref()),
            ("Model", self.model.as_ref()),
            ("Serial", self.serial.as_ref()),
            ("Technology", self.technology.as_ref()),
        ])
    }
}

impl HardwareRows for ModemIdentity {
    fn hardware_rows(&self) -> Vec<Row> {
        let mut rows = reported_rows([
            ("Manufacturer", self.manufacturer.as_ref()),
            ("Model", self.model.as_ref()),
            ("Firmware", self.revision.as_ref()),
        ]);
        rows.extend(
            self.adapter
                .as_ref()
                .map(Adapter::bus_rows)
                .unwrap_or_default(),
        );
        rows
    }
}

impl HardwareRows for Adapter {
    fn hardware_rows(&self) -> Vec<Row> {
        let mut rows = reported_rows([
            ("Vendor", self.vendor.as_ref()),
            ("Model", self.model.as_ref()),
        ]);
        rows.extend(self.bus_rows());
        rows
    }
}

impl Adapter {
    fn bus_rows(&self) -> Vec<Row> {
        let mut rows = vec![row(self.bus_id.bus_name(), self.bus_id.to_string())];
        rows.extend(reported_rows([("Driver", self.driver.as_ref())]));
        rows.push(link("Lookup", self.bus_id.lookup_url()));
        rows
    }
}
