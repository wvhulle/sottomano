pub mod bluetooth_adapters_group;
mod echo_probe_window_rows;
pub mod ethernet_ports_group;
mod interface_address_sections;
mod interface_group_titles;
pub mod mobile_modems_group;
pub mod network_manager_vpn_group;
pub mod reachability_group;
pub mod tailnet_group;
pub mod wifi_adapters_group;
pub mod wireguard_tunnels_group;

use sottomano_toml_config::secrets::StoredSecret;

use self::{
    bluetooth_adapters_group::BluetoothAdaptersGroup, ethernet_ports_group::EthernetPortsGroup,
    mobile_modems_group::MobileModemsGroup, network_manager_vpn_group::NetworkManagerVpnGroup,
    reachability_group::ReachabilityGroup, tailnet_group::TailnetGroup,
    wifi_adapters_group::WifiAdaptersGroup, wireguard_tunnels_group::WireguardTunnelsGroup,
};
use crate::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
    panel_menus::gaming_menu::gamepad_group::GamepadGroup,
};

pub struct NetworkMenu {
    pub wifi_adapters_group: WifiAdaptersGroup,
    pub mobile_modems_group: MobileModemsGroup,
    pub ethernet_ports_group: EthernetPortsGroup,
    pub bluetooth_adapters_group: BluetoothAdaptersGroup,
    pub reachability_group: ReachabilityGroup,
    pub tailnet_group: TailnetGroup,
    pub wireguard_tunnels_group: WireguardTunnelsGroup,
    pub network_manager_vpn_group: NetworkManagerVpnGroup,
}

impl NetworkMenu {
    pub fn collapse(&mut self) {
        self.wifi_adapters_group.collapse();
        self.mobile_modems_group.collapse();
        self.ethernet_ports_group.collapse();
        self.bluetooth_adapters_group.collapse();
        self.reachability_group.collapse();
        self.tailnet_group.collapse();
        self.wireguard_tunnels_group.collapse();
        self.network_manager_vpn_group.collapse();
    }

    #[must_use]
    pub fn new(tailscale_api_token: StoredSecret) -> Self {
        Self {
            wifi_adapters_group: WifiAdaptersGroup::default(),
            mobile_modems_group: MobileModemsGroup::default(),
            ethernet_ports_group: EthernetPortsGroup::default(),
            bluetooth_adapters_group: BluetoothAdaptersGroup::default(),
            reachability_group: ReachabilityGroup::default(),
            tailnet_group: match tailscale_api_token {
                StoredSecret::Present(token_path) => TailnetGroup::with_policy_file(token_path),
                StoredSecret::Absent => TailnetGroup::without_policy_file(),
            },
            wireguard_tunnels_group: WireguardTunnelsGroup::default(),
            network_manager_vpn_group: NetworkManagerVpnGroup::default(),
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        gamepad_group: &'view GamepadGroup,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        self.wifi_adapters_group
            .group_contents()
            .into_iter()
            .chain(self.mobile_modems_group.group_contents())
            .chain(self.ethernet_ports_group.group_contents())
            .chain(self.bluetooth_adapters_group.group_contents(
                &|hardware_address| gamepad_group.report_timing_for_address(hardware_address),
                clock,
            ))
            .chain([
                self.reachability_group.group_content(),
                self.tailnet_group.group_content(clock),
                self.wireguard_tunnels_group.group_content(),
                self.network_manager_vpn_group.group_content(),
            ])
            .collect()
    }
}
