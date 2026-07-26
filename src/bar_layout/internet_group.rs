//! odin's `InternetConnectivity`: a downlink throughput gauge on the bar that
//! unfolds an uplink gauge, one pill per active `WireGuard` tunnel, and a
//! tailscale pill. The downlink handle both shows live throughput and toggles
//! the menu; hovering any element shows its tooltip.
//!
//! Deviations: odin's generic `NetworkManager` VPN shield (for non-WireGuard
//! VPNs) is not ported, and the per-tunnel/tailscale pills act on right/left
//! click but have no drag slider (there is nothing to drag).

use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, mouse_area},
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget, Tooltip,
        tailscale::{Tailscale, open_admin},
        throughput::Throughput,
        wireguard::{Wireguard, disconnect},
    },
    layer_surface::Position,
    tooltip::{TooltipMessage, hoverable},
    widgets::{
        icon_gauge::source_reading,
        menu_group::{MenuGroup, MenuMessage},
    },
};

pub struct InternetGroup {
    down: SourceCell<Throughput<false>>,
    up: SourceCell<Throughput<true>>,
    wireguard: SourceCell<Wireguard>,
    tailscale: SourceCell<Tailscale>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Down(Option<Throughput<false>>),
    Up(Option<Throughput<true>>),
    Wireguard(Option<Wireguard>),
    Tailscale(Option<Tailscale>),
    Menu(MenuMessage),
    Disconnect(String),
    OpenTailscale,
    Tooltip(TooltipMessage),
}

impl InternetGroup {
    pub fn new(menu_expand_ms: u64) -> Self {
        Self {
            down: SourceCell::default(),
            up: SourceCell::default(),
            wireguard: SourceCell::default(),
            tailscale: SourceCell::default(),
            menu: MenuGroup::new(menu_expand_ms),
        }
    }

    pub fn subscription() -> Subscription<InternetMessage> {
        Subscription::batch([
            SourceCell::subscription().map(InternetMessage::Down),
            SourceCell::subscription().map(InternetMessage::Up),
            SourceCell::subscription().map(InternetMessage::Wireguard),
            SourceCell::subscription().map(InternetMessage::Tailscale),
        ])
    }

    pub fn update(&mut self, msg: InternetMessage) -> Task<InternetMessage> {
        use crate::message_subscriptions::blocking;
        match msg {
            InternetMessage::Down(reading) => self.down.load(reading),
            InternetMessage::Up(reading) => self.up.load(reading),
            InternetMessage::Wireguard(reading) => self.wireguard.load(reading),
            InternetMessage::Tailscale(reading) => self.tailscale.load(reading),
            InternetMessage::Menu(m) => {
                self.menu.update(m);
            }
            InternetMessage::Disconnect(name) => {
                return blocking("wireguard down", move || disconnect(&name)).discard();
            }
            InternetMessage::OpenTailscale => {
                return blocking("tailscale admin", open_admin).discard();
            }
            InternetMessage::Tooltip(_) => {}
        }
        Task::none()
    }

    pub fn tooltip_for(&self, id: &str) -> Option<Tooltip> {
        // Each tunnel is keyed by its (unique) interface name, built from the
        // tunnel's own body — the aggregate cell tooltip is inert.
        if let Some(name) = id.strip_prefix("wireguard-") {
            let tunnel = self.wireguard.get()?.tunnels.iter().find(|t| t.name == name)?;
            return Some(Tooltip {
                body: tunnel.tooltip(),
                footer: None,
            });
        }
        match id {
            "internet-down" => self.down.tooltip(),
            "internet-up" => self.up.tooltip(),
            "tailscale" => self.tailscale.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &ResolvedConfig,
        position: Position,
    ) -> Option<Element<'a, InternetMessage>> {
        // The downlink gauge is the persistent handle; without it the group hides.
        let down = self.down.get()?;
        let handle_reading = source_reading(config, Some(down as &dyn BarWidget), 1.0)?;
        let handle = mouse_area(hoverable(
            handle_reading,
            "internet-down",
            InternetMessage::Tooltip,
        ))
        .on_press(InternetMessage::Menu(MenuMessage::Toggle))
        .into();

        let reveal = self.menu.reveal();
        let mut pills: Vec<Element<'a, InternetMessage>> = Vec::new();
        if let Some(up) = self.up.get()
            && let Some(reading) = source_reading(config, Some(up as &dyn BarWidget), reveal)
        {
            pills.push(hoverable(reading, "internet-up", InternetMessage::Tooltip));
        }
        if let Some(wireguard) = self.wireguard.get() {
            for tunnel in &wireguard.tunnels {
                if let Some(reading) =
                    source_reading(config, Some(tunnel as &dyn BarWidget), reveal)
                {
                    let name = tunnel.name.clone();
                    let hover = hoverable(
                        reading,
                        format!("wireguard-{name}"),
                        InternetMessage::Tooltip,
                    );
                    pills.push(
                        mouse_area(hover)
                            .on_right_press(InternetMessage::Disconnect(name))
                            .into(),
                    );
                }
            }
        }
        if let Some(tailscale) = self.tailscale.get()
            && let Some(reading) = source_reading(config, Some(tailscale as &dyn BarWidget), reveal)
        {
            let hover = hoverable(reading, "tailscale", InternetMessage::Tooltip);
            pills.push(
                mouse_area(hover)
                    .on_press(InternetMessage::OpenTailscale)
                    .into(),
            );
        }

        let content = Row::with_children(pills)
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center);

        Some(self.menu.view(
            config,
            position,
            reveal,
            handle,
            content.into(),
            InternetMessage::Menu,
        ))
    }
}
