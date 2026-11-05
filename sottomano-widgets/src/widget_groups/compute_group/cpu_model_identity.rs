use chrono::Locale;
use sottomano_desktop_sources::host_machine_identity::udev_hardware_identity::CpuModel;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
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

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::from(self)])
    }
}
