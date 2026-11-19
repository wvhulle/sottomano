use sottomano_desktop_sources::network_connectivity::network_manager_vpn_connections::{
    VpnActivity, VpnConnection, VpnState,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
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
    let profile_entries = [
        Some(LabeledValue::new(
            connection.name.as_str(),
            connection.protocol_label(),
        )),
        connection
            .remote_gateway
            .as_deref()
            .map(|gateway| LabeledValue::new("Gateway", gateway)),
    ];
    let VpnActivity::Active(tunnel) = &connection.activity else {
        return vec![Section::new(
            "Connection",
            dictionary_table(
                profile_entries
                    .into_iter()
                    .flatten()
                    .chain([LabeledValue::new("State", "not connected")]),
            ),
        )];
    };
    let state_entry = LabeledValue::new("State", tunnel.vpn_state.label());
    let tunnel_entries = [
        Some(match tunnel.vpn_state {
            VpnState::Failed => state_entry.level(Severity::Warning),
            _ => state_entry,
        }),
        tunnel
            .ipv4
            .as_ref()
            .map(|block| LabeledValue::new("IPv4", block.address.as_str())),
        tunnel
            .ipv6
            .as_ref()
            .map(|block| LabeledValue::new("IPv6", block.address.as_str())),
    ];
    let dns_entries = tunnel
        .dns
        .iter()
        .map(|nameserver| LabeledValue::new("DNS", nameserver.as_str()));
    let banner_entries = tunnel
        .banner
        .as_deref()
        .map(|banner| LabeledValue::new("Banner", banner));
    vec![
        Section::new(
            "Connection",
            dictionary_table(
                profile_entries
                    .into_iter()
                    .chain(tunnel_entries)
                    .flatten()
                    .chain(dns_entries),
            ),
        ),
        Section::new("Banner", dictionary_table(banner_entries)),
    ]
}
