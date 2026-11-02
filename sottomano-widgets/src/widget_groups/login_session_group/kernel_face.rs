use chrono::Locale;
use sottomano_sources::host_machine_identity::hostname1_os_identity::{
    Chassis, OperatingSystemIdentity,
};

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
        vendor_logo::{DistributionIds, VendorLogo},
    },
};

pub struct KernelFace<'a> {
    pub system: &'a OperatingSystemIdentity,
}

impl GroupItemReading for KernelFace<'_> {
    fn name(&self) -> &'static str {
        "Kernel"
    }

    fn icon(&self) -> IconGlyph {
        DistributionIds(&self.system.distribution_ids)
            .vendor_logo()
            .map_or_else(
                || Phosphor::LinuxLogo(Weight::Regular).into(),
                IconGlyph::from,
            )
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.system.kernel.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            [
                Some(LabeledValue::new("Host", self.system.hostname.clone())),
                Some(LabeledValue::new("Kernel", self.system.kernel.clone())),
                Some(LabeledValue::new(
                    "OS",
                    self.system.operating_system.clone(),
                )),
                self.system
                    .chassis
                    .map(Chassis::stored_name)
                    .map(|chassis_kind| LabeledValue::new("Chassis", chassis_kind)),
                Some(LabeledValue::new(
                    "Wiki",
                    ValueCell::BrandedLink {
                        label: "Kernel".to_owned(),
                        url: "https://wiki.archlinux.org/title/Kernel".to_owned(),
                        glyph: SimpleIcon::ArchLinux.into(),
                    },
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((
            "Host and kernel under this session",
            "The kernel drives this host's hardware and runs every process above it; a new \
             version takes effect after a reboot.",
        ))])
    }
}
