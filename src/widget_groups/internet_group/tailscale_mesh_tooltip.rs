use crate::{
    data::source::tailscale::Tailscale,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
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
            return TooltipBody::sections(&[Section::untitled_dictionary([KeyValue::new(
                "Tailscale",
                "disconnected",
            )])]);
        }
        let online_peers = self
            .online
            .iter()
            .map(|peer| KeyValue::new(peer.name.as_str(), format!("→ {}", peer.ip)));
        TooltipBody::sections(&[
            Section::untitled_dictionary([
                KeyValue::new("Tailscale", "connected"),
                KeyValue::new("Self", self.self_name.as_str()),
                KeyValue::new("IP", self.self_ip.as_str()),
                KeyValue::new(
                    "Peers",
                    format!("{}/{} online", self.online.len(), self.total_peers),
                ),
            ]),
            Section::untitled_dictionary(online_peers),
        ])
    }
}
