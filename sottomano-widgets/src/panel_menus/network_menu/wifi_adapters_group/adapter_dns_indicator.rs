use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::LinkPhase,
    wifi_connection::{AdapterAddressing, WifiAdapter},
};

use super::WifiAdaptersGroup;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_address_sections::{
        InterfaceDnsIndicator, dns_server_sections,
    },
};

impl WifiAdaptersGroup {
    pub(super) fn adapter_dns_rows<'view>(
        &'view self,
        adapter: &'view WifiAdapter,
    ) -> Vec<GroupIndicator<'view>> {
        if adapter.link_state != LinkPhase::Up {
            return Vec::new();
        }
        vec![
            InterfaceDnsIndicator::indicator()
                .with_hover_card(Box::new(move |_| self.adapter_dns_hover_card(adapter))),
        ]
    }

    fn adapter_dns_hover_card(&self, adapter: &WifiAdapter) -> Vec<Section> {
        self.adapter_detail_sections(&adapter.device, |details| {
            let AdapterAddressing::Addressed(addressing) = &details.addressing else {
                return Vec::new();
            };
            dns_server_sections(&addressing.dns)
        })
    }
}
