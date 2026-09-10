use crate::{
    data::source::tailscale::Tailscale,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Tailscale {
    fn icon(&self) -> Glyph {
        Solid::CircleNodes.into()
    }

    fn disabled(&self) -> bool {
        !self.running
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
            return TooltipBody::rows(vec![row("Tailscale", "disconnected")]);
        }
        let mut rows = vec![
            row("Tailscale", "connected"),
            row("Self", self.self_name.clone()),
            row("IP", self.self_ip.clone()),
            row(
                "Peers",
                format!("{}/{} online", self.online.len(), self.total_peers),
            ),
        ];
        rows.extend(
            self.online
                .iter()
                .map(|peer| row(&peer.name, format!("→ {}", peer.ip))),
        );
        TooltipBody::rows(rows)
    }
}
