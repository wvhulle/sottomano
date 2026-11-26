use sottomano_desktop_sources::network_connectivity::{
    ethernet_connection::{PortLink, PortState},
    network_manager_proxies::LinkPhase,
};

use super::EthernetPortsGroup;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_address_sections::{
        InterfaceDnsIndicator, dns_server_sections,
    },
};

impl EthernetPortsGroup {
    pub(super) fn port_dns_rows<'view>(
        &'view self,
        port_state: &'view PortState,
    ) -> Vec<GroupIndicator<'view>> {
        if port_state.link_state != LinkPhase::Up {
            return Vec::new();
        }
        vec![
            InterfaceDnsIndicator::indicator()
                .with_hover_card(Box::new(move |_| self.interface_dns_hover_card(port_state))),
        ]
    }

    fn interface_dns_hover_card(&self, port_state: &PortState) -> Vec<Section> {
        self.port_detail_sections(&port_state.name, |port| {
            let PortLink::Up(link) = &port.link else {
                return Vec::new();
            };
            dns_server_sections(&link.dns)
        })
    }
}
