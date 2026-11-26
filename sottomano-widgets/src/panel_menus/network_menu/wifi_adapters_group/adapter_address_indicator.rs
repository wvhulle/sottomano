use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::LinkPhase,
    wifi_connection::{AdapterAddressing, WifiAdapter},
};

use super::WifiAdaptersGroup;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    panel_menus::network_menu::interface_address_sections::{
        AddressFamily, InterfaceAddressIndicator, family_address_sections, family_router_sections,
    },
};

impl WifiAdaptersGroup {
    pub(super) fn adapter_address_rows<'view>(
        &'view self,
        adapter: &'view WifiAdapter,
    ) -> Vec<GroupIndicator<'view>> {
        if adapter.link_state != LinkPhase::Up {
            return Vec::new();
        }
        [AddressFamily::Ipv4, AddressFamily::Ipv6]
            .map(|family| {
                InterfaceAddressIndicator { family }
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.adapter_address_hover_card(adapter, family)
                    }))
            })
            .into()
    }

    fn adapter_address_hover_card(
        &self,
        adapter: &WifiAdapter,
        family: AddressFamily,
    ) -> Vec<Section> {
        let address_details = self.adapter_detail_sections(&adapter.device, |details| {
            let AdapterAddressing::Addressed(addressing) = &details.addressing else {
                return Vec::new();
            };
            family_address_sections(family, &addressing.ipv4, &addressing.ipv6)
        });
        let router_details = self.gateway_latencies.sections_described_by(|latencies| {
            family_router_sections(latencies, &adapter.device, family)
        });
        address_details.into_iter().chain(router_details).collect()
    }
}
