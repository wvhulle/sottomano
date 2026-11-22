use super::IndicatorGroupInteraction;
use crate::indicator_groups::{
    bluetooth_adapters_group::BluetoothInteraction, ethernet_ports_group::EthernetPortsInteraction,
    mobile_modems_group::MobileModemsInteraction,
    network_manager_vpn_group::NetworkManagerVpnInteraction,
    reachability_group::ReachabilityInteraction, steam_client_groups::SteamClientInteraction,
    tailnet_group::TailnetInteraction, wifi_adapters_group::WifiAdaptersInteraction,
    wireguard_tunnels_group::WireguardTunnelsInteraction,
};

impl From<ReachabilityInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ReachabilityInteraction) -> Self {
        IndicatorGroupInteraction::Reachability(interaction)
    }
}

impl From<TailnetInteraction> for IndicatorGroupInteraction {
    fn from(interaction: TailnetInteraction) -> Self {
        IndicatorGroupInteraction::Tailnet(interaction)
    }
}

impl From<WireguardTunnelsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WireguardTunnelsInteraction) -> Self {
        IndicatorGroupInteraction::WireguardTunnels(interaction)
    }
}

impl From<NetworkManagerVpnInteraction> for IndicatorGroupInteraction {
    fn from(interaction: NetworkManagerVpnInteraction) -> Self {
        IndicatorGroupInteraction::NetworkManagerVpns(interaction)
    }
}

impl From<WifiAdaptersInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WifiAdaptersInteraction) -> Self {
        IndicatorGroupInteraction::WifiAdapters(interaction)
    }
}

impl From<SteamClientInteraction> for IndicatorGroupInteraction {
    fn from(interaction: SteamClientInteraction) -> Self {
        IndicatorGroupInteraction::SteamClient(interaction)
    }
}

impl From<MobileModemsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MobileModemsInteraction) -> Self {
        IndicatorGroupInteraction::MobileModems(interaction)
    }
}

impl From<EthernetPortsInteraction> for IndicatorGroupInteraction {
    fn from(interaction: EthernetPortsInteraction) -> Self {
        IndicatorGroupInteraction::EthernetPorts(interaction)
    }
}

impl From<BluetoothInteraction> for IndicatorGroupInteraction {
    fn from(interaction: BluetoothInteraction) -> Self {
        IndicatorGroupInteraction::BluetoothAdapters(interaction)
    }
}
