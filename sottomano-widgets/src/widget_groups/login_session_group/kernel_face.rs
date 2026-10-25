use chrono::Locale;
use sottomano_sources::host_machine_identity::hostname1_os_identity::{
    Chassis, OperatingSystemIdentity,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody, entry},
        vendor_logo::{DistributionIds, VendorLogo},
    },
};

pub struct KernelFace<'a> {
    pub system: &'a OperatingSystemIdentity,
}

impl BarWidget for KernelFace<'_> {
    fn name(&self) -> &'static str {
        "Kernel"
    }

    fn icon(&self) -> Glyph {
        DistributionIds(&self.system.distribution_ids)
            .vendor_logo()
            .map_or_else(|| Phosphor::LinuxLogo(Weight::Regular).into(), Glyph::from)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.system.kernel.clone())
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::dictionary(
            [
                Some(KeyValue::new("Host", self.system.hostname.clone())),
                Some(KeyValue::new("Kernel", self.system.kernel.clone())),
                Some(KeyValue::new("OS", self.system.operating_system.clone())),
                entry("Chassis", self.system.chassis.map(Chassis::stored_name)),
                Some(KeyValue::new(
                    "Wiki",
                    Cell::Link {
                        label: "Kernel".to_owned(),
                        url: "https://wiki.archlinux.org/title/Kernel".to_owned(),
                        glyph: Some(SimpleIcon::ArchLinux.into()),
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
