use sottomano_sources::network_connectivity::tailscale_status::Tailscale;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Tailscale {
    fn name(&self) -> &'static str {
        "Tailscale"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::Tailscale.into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(matches!(self, Tailscale::Running(_)))
    }

    fn badge(&self) -> Option<u32> {
        let Tailscale::Running(tailnet) = self else {
            return None;
        };
        u32::try_from(tailnet.online.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let Tailscale::Running(tailnet) = self else {
            return TooltipBody::dictionary([KeyValue::new("Tailscale", "disconnected")]);
        };
        let own_ip = tailnet
            .own_node
            .ip
            .as_deref()
            .map(|ip| KeyValue::new("IP", ip));
        let online_peers = tailnet.online.iter().filter_map(|peer| {
            let ip = peer.ip.as_deref()?;
            Some(KeyValue::new(peer.name.as_str(), format!("→ {ip}")))
        });
        TooltipBody::sections([
            Section::dictionary(
                [
                    KeyValue::new("Tailscale", "connected"),
                    KeyValue::new("Self", tailnet.own_node.name.as_str()),
                ]
                .into_iter()
                .chain(own_ip)
                .chain([KeyValue::new(
                    "Peers",
                    format!("{}/{} online", tailnet.online.len(), tailnet.total_peers),
                )]),
            ),
            Section::dictionary(online_peers),
        ])
    }
}
