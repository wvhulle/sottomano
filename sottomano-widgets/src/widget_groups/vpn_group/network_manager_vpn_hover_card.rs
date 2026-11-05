use sottomano_desktop_sources::network_connectivity::network_manager_vpn_connections::{
    NetworkManagerVpns, VpnActivity, VpnConnection, VpnState,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for NetworkManagerVpns {
    fn name(&self) -> &'static str {
        "NetworkManager VPNs"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::LockKey(Weight::Regular).into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
    }
}

impl GroupItemReading for VpnConnection {
    fn name(&self) -> &'static str {
        "VPN connection"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::LockKey(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match &self.activity {
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
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let profile_entries = [
            Some(LabeledValue::new(self.name.as_str(), self.protocol_label())),
            self.remote_gateway
                .as_deref()
                .map(|gateway| LabeledValue::new("Gateway", gateway)),
        ];
        let VpnActivity::Active(tunnel) = &self.activity else {
            return HoverCardBody::sections([Section::dictionary(
                profile_entries
                    .into_iter()
                    .flatten()
                    .chain([LabeledValue::new("State", "not connected")]),
            )]);
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
        HoverCardBody::sections([
            Section::dictionary(
                profile_entries
                    .into_iter()
                    .chain(tunnel_entries)
                    .flatten()
                    .chain(dns_entries),
            ),
            Section::dictionary(banner_entries),
        ])
    }
}
