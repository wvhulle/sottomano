use crate::{
    data::source::tailscale::{Peer, Tailnet, Tailscale},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Tailscale {
    fn name(&self) -> &'static str {
        "Tailscale"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Graph(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
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

    fn tooltip(&self) -> TooltipBody {
        match self {
            Tailscale::Stopped => {
                TooltipBody::sections(&[Section::untitled_dictionary([KeyValue::new(
                    "Tailscale",
                    "disconnected",
                )])])
            }
            Tailscale::Running(tailnet) => tailnet.tooltip(),
        }
    }
}

impl Tailnet {
    fn tooltip(&self) -> TooltipBody {
        let own_ip = self
            .own_node
            .ip
            .as_deref()
            .map(|ip| KeyValue::new("IP", ip));
        TooltipBody::sections(&[
            Section::untitled_dictionary(
                [
                    KeyValue::new("Tailscale", "connected"),
                    KeyValue::new("Self", self.own_node.name.as_str()),
                ]
                .into_iter()
                .chain(own_ip)
                .chain([KeyValue::new(
                    "Peers",
                    format!("{}/{} online", self.online.len(), self.total_peers),
                )]),
            ),
            Section::untitled_dictionary(self.online.iter().filter_map(Peer::key_value)),
        ])
    }
}

impl Peer {
    fn key_value(&self) -> Option<KeyValue> {
        let ip = self.ip.as_deref()?;
        Some(KeyValue::new(self.name.as_str(), format!("→ {ip}")))
    }
}
