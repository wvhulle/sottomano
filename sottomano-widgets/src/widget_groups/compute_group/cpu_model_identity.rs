use chrono::Locale;
use sottomano_sources::host_machine_identity::udev_hardware_identity::CpuModel;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        vendor_logo::VendorLogo,
    },
};

impl GroupItemReading for CpuModel {
    fn visible_on_panel(&self) -> bool {
        self.identity.name.is_some()
    }

    fn name(&self) -> &'static str {
        "CPU model"
    }

    fn icon(&self) -> IconGlyph {
        self.identity
            .vendor
            .and_then(|vendor| vendor.vendor_logo())
            .map_or_else(
                || Phosphor::IdentificationCard(Weight::Regular).into(),
                IconGlyph::from,
            )
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.architecture.microarchitecture.clone()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::from(self)])
    }
}
