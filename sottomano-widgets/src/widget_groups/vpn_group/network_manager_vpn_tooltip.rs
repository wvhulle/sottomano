use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::network_manager_vpn_connections::{
    NetworkManagerVpns, VpnActivity, VpnConnection, VpnState,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
        widget_state::WidgetState,
    },
};

impl BarWidget for NetworkManagerVpns {
    fn name(&self) -> &'static str {
        "NetworkManager VPNs"
    }

    fn icon(&self) -> Glyph {
        Phosphor::LockKey(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl BarWidget for VpnConnection {
    fn name(&self) -> &'static str {
        "VPN connection"
    }

    fn icon(&self) -> Glyph {
        Phosphor::LockKey(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match &self.activity {
            VpnActivity::Inactive => WidgetState::Off,
            VpnActivity::Active(tunnel) => match tunnel.vpn_state {
                VpnState::Activated => WidgetState::On,
                VpnState::Failed => WidgetState::Warning,
                VpnState::Disconnected => WidgetState::Off,
                VpnState::Preparing
                | VpnState::NeedsAuthentication
                | VpnState::Connecting
                | VpnState::AwaitingAddress => WidgetState::Pending,
            },
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let profile_entries = [
            Some(LabeledValue::new(self.name.as_str(), self.protocol_label())),
            self.remote_gateway
                .as_deref()
                .map(|gateway| LabeledValue::new("Gateway", gateway)),
        ];
        let VpnActivity::Active(tunnel) = &self.activity else {
            return TooltipBody::sections([Section::dictionary(
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
        TooltipBody::sections([
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
