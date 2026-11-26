use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{AddressAssignment, NmConnectionSwitch},
    network_manager_vpn_connections::{
        ServerBanner, VpnActivity, VpnConnection, VpnRemoteGateway, VpnState,
    },
};
use sottomano_toml_config::severity::Severity;

use super::{NetworkManagerVpnBusRequest, NetworkManagerVpnGroup, NetworkManagerVpnInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub fn vpn_connection_indicator(connection: &VpnConnection) -> GroupIndicator<'_> {
    let state = match &connection.activity {
        VpnActivity::Inactive => IndicatorState::Off,
        VpnActivity::Active(tunnel) => match tunnel.vpn_state {
            VpnState::Activated => IndicatorState::On,
            VpnState::Failed => IndicatorState::Warning,
            VpnState::Disconnected => IndicatorState::Off,
            VpnState::Preparing
            | VpnState::NeedsAuthentication
            | VpnState::Connecting
            | VpnState::AwaitingAddress => IndicatorState::Pending,
        },
    };
    GroupIndicator::glyph_only(
        connection.name.clone(),
        Phosphor::LockKey(Weight::Regular).into(),
        state,
    )
    .with_hover_card(Box::new(move |_| vpn_connection_hover_card(connection)))
}

pub fn vpn_connection_hover_card(connection: &VpnConnection) -> Vec<Section> {
    let mut connection_entries = vec![LabeledValue::new(
        connection.name.as_str(),
        connection.protocol_label(),
    )];
    if let VpnRemoteGateway::Configured(gateway) = &connection.remote_gateway {
        connection_entries.push(LabeledValue::new("Gateway", gateway.as_str()));
    }
    let VpnActivity::Active(tunnel) = &connection.activity else {
        connection_entries.push(LabeledValue::new("State", "not connected"));
        return vec![Section::new(
            "Connection",
            dictionary_table(connection_entries),
        )];
    };
    let state_entry = LabeledValue::new("State", tunnel.vpn_state.label());
    connection_entries.push(match tunnel.vpn_state {
        VpnState::Failed => state_entry.level(Severity::Warning),
        _ => state_entry,
    });
    if let AddressAssignment::Assigned(block) = &tunnel.ipv4 {
        connection_entries.push(LabeledValue::new("IPv4", block.address.as_str()));
    }
    if let AddressAssignment::Assigned(block) = &tunnel.ipv6 {
        connection_entries.push(LabeledValue::new("IPv6", block.address.as_str()));
    }
    connection_entries.extend(
        tunnel
            .dns
            .iter()
            .map(|nameserver| LabeledValue::new("DNS", nameserver.as_str())),
    );
    let banner_entries = match &tunnel.banner {
        ServerBanner::Message(banner) => vec![LabeledValue::new("Banner", banner.as_str())],
        ServerBanner::Silent => Vec::new(),
    };
    vec![
        Section::new("Connection", dictionary_table(connection_entries)),
        Section::new("Banner", dictionary_table(banner_entries)),
    ]
}

impl NetworkManagerVpnGroup {
    pub(super) fn vpn_connection_rows(&self) -> Vec<GroupIndicator<'_>> {
        self.vpn_connections()
            .iter()
            .map(move |vpn_connection| self.wired_vpn_connection_indicator(vpn_connection))
            .collect()
    }

    fn wired_vpn_connection_indicator<'view>(
        &'view self,
        vpn_connection: &'view VpnConnection,
    ) -> GroupIndicator<'view> {
        let vpn_switch = vpn_connection.switch();
        vpn_connection_indicator(vpn_connection)
            .on_switch_press(
                match vpn_switch {
                    NmConnectionSwitch::Connect(_) => SwitchPosition::Off,
                    NmConnectionSwitch::Disconnect(_) => SwitchPosition::On,
                },
                match vpn_switch {
                    NmConnectionSwitch::Connect(_) => "connect the VPN",
                    NmConnectionSwitch::Disconnect(_) => "disconnect the VPN",
                },
                NetworkManagerVpnInteraction::SwitchVpn(vpn_switch),
            )
            .with_hover_card(Box::new(move |_| {
                self.network_manager_vpns
                    .hover_card_with(vpn_connection_hover_card(vpn_connection))
            }))
            .refreshed_by(NetworkManagerVpnInteraction::OnBuses(
                NetworkManagerVpnBusRequest::Refresh,
            ))
    }
}
