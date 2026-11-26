use sottomano_desktop_sources::network_connectivity::{
    modem_manager_mobile::{BearerAddress, Mobile},
    network_manager_proxies::LinkPhase,
};

use super::MobileModemsGroup;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::network_menu::interface_address_sections::{
        AddressFamily, InterfaceAddressIndicator,
    },
    widgets::value_cells::LabeledValue,
};

#[must_use]
pub fn modem_family_address(modem: &Mobile, family: AddressFamily) -> &BearerAddress {
    match family {
        AddressFamily::Ipv4 => &modem.ipv4,
        AddressFamily::Ipv6 => &modem.ipv6,
    }
}

impl MobileModemsGroup {
    pub(super) fn modem_address_rows<'view>(
        &'view self,
        modem: &'view Mobile,
    ) -> impl Iterator<Item = GroupIndicator<'view>> {
        [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .into_iter()
            .filter(move |family| {
                modem.link_state() == LinkPhase::Up
                    && matches!(
                        modem_family_address(modem, *family),
                        BearerAddress::Assigned(_)
                    )
            })
            .map(move |family| {
                InterfaceAddressIndicator { family }
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.modem_address_hover_card(modem, family)
                    }))
            })
    }

    fn modem_address_hover_card(&self, modem: &Mobile, family: AddressFamily) -> Vec<Section> {
        let BearerAddress::Assigned(address) = modem_family_address(modem, family) else {
            return Vec::new();
        };
        self.mobile_modems.hover_card_with(vec![Section::new(
            "Address",
            dictionary_table([LabeledValue::new("Address", address.as_str())]),
        )])
    }
}
