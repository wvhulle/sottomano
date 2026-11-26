use sottomano_desktop_sources::network_connectivity::{
    ethernet_connection::{PortLink, PortState},
    network_manager_proxies::LinkPhase,
};

use super::EthernetPortsGroup;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_address_sections::{
        AddressFamily, InterfaceAddressIndicator, family_address_sections, family_router_sections,
    },
};

impl EthernetPortsGroup {
    pub(super) fn port_address_rows<'view>(
        &'view self,
        port_state: &'view PortState,
    ) -> Vec<GroupIndicator<'view>> {
        if port_state.link_state != LinkPhase::Up {
            return Vec::new();
        }
        [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .map(|family| {
                InterfaceAddressIndicator { family }
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.interface_address_hover_card(port_state, family)
                    }))
            })
            .into()
    }

    fn interface_address_hover_card(
        &self,
        port_state: &PortState,
        family: AddressFamily,
    ) -> Vec<Section> {
        let address_details = self.port_detail_sections(&port_state.name, |port| {
            let PortLink::Up(link) = &port.link else {
                return Vec::new();
            };
            family_address_sections(family, &link.ipv4, &link.ipv6)
        });
        let router_details = self.gateway_latencies.sections_described_by(|latencies| {
            family_router_sections(latencies, &port_state.name, family)
        });
        address_details.into_iter().chain(router_details).collect()
    }
}
