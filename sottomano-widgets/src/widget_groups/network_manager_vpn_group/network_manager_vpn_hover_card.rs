use sottomano_desktop_sources::network_connectivity::network_manager_vpn_connections::{
    VpnActivity, VpnConnection, VpnState,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{reading_display_state::ReadingDisplayState, reading_row::ReadingRow},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub fn vpn_connection_reading_row(connection: &VpnConnection) -> ReadingRow<'_> {
    let state = match &connection.activity {
        VpnActivity::Inactive => ReadingDisplayState::Off,
        VpnActivity::Active(tunnel) => match tunnel.vpn_state {
            VpnState::Activated => ReadingDisplayState::On,
            VpnState::Failed => ReadingDisplayState::Warning,
            VpnState::Disconnected => ReadingDisplayState::Off,
            VpnState::Preparing
            | VpnState::NeedsAuthentication
            | VpnState::Connecting
            | VpnState::AwaitingAddress => ReadingDisplayState::Pending,
        },
    };
    ReadingRow::glyph_only(
        connection.name.clone(),
        Phosphor::LockKey(Weight::Regular).into(),
        state,
        Box::new(move |clock| vpn_connection_hover_card(connection, clock)),
    )
}

pub fn vpn_connection_hover_card(connection: &VpnConnection, _clock: Clock) -> Vec<Section> {
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
