use crate::{
    data::source::tailscale::Tailscale,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Tailscale {
    fn icon(&self) -> Glyph {
        Phosphor::Graph(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.running)
    }

    fn badge(&self) -> Option<u32> {
        if !self.running {
            return None;
        }
        u32::try_from(self.online.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.running {
            return TooltipBody::sections(&[Section::untitled(&[("Tailscale", "disconnected")])]);
        }
        let online_peers: Vec<(&str, String)> = self
            .online
            .iter()
            .map(|peer| (peer.name.as_str(), format!("→ {}", peer.ip)))
            .collect();
        TooltipBody::sections(&[
            Section::untitled(&[
                ("Tailscale", "connected".to_owned()),
                ("Self", self.self_name.clone()),
                ("IP", self.self_ip.clone()),
                (
                    "Peers",
                    format!("{}/{} online", self.online.len(), self.total_peers),
                ),
            ]),
            Section::untitled(&online_peers),
        ])
    }
}
