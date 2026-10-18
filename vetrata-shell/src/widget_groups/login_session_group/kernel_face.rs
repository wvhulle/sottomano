use crate::{
    data::source::hostname::{Chassis, OperatingSystemIdentity, entry},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
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
        Phosphor::LinuxLogo(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(self.system.kernel.clone())
    }

    fn tooltip(&self) -> TooltipBody {
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
