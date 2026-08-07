use iced::{
    Alignment, Element, Subscription, Task,
    widget::{Row, button, mouse_area},
};

use crate::{
    config::Config,
    data::source::{
        BarWidget, Tooltip,
        tailscale::{Tailscale, open_admin},
        throughput::{self, Readings, Throughput},
        wireguard::{Wireguard, disconnect},
    },
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::source_cell::SourceCell,
    widgets::{
        flat_button,
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
    Throughput(Readings),
    Wireguard(Option<Wireguard>),
    Tailscale(Option<Tailscale>),
    Menu(MenuMessage),
    Disconnect(String),
    OpenTailscale,
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for InternetMessage {
    fn into_tooltip(self) -> Result<TooltipMessage, Self> {
        match self {
            Self::Tooltip(tooltip) => Ok(tooltip),
            message => Err(message),
        }
    }

    fn from_tooltip(tooltip: TooltipMessage) -> Self {
        Self::Tooltip(tooltip)
    }
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
            throughput::subscription().map(InternetMessage::Throughput),
            SourceCell::subscription().map(InternetMessage::Wireguard),
            SourceCell::subscription().map(InternetMessage::Tailscale),
        ])
    }

    pub fn update(&mut self, msg: InternetMessage) -> Task<InternetMessage> {
        use crate::message_subscriptions::blocking;
        match msg {
            InternetMessage::Throughput(readings) => {
                self.down.load(Some(readings.down));
                self.up.load(Some(readings.up));
            }
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Wireguard(name) => {
                let tunnel = self
                    .wireguard
                    .get()?
                    .tunnels
                    .iter()
                    .find(|t| &t.name == name)?;
                Some(Tooltip::of(tunnel.tooltip()))
            }
            HoverTarget::InternetDown => self.down.live_tooltip(),
            HoverTarget::InternetUp => self.up.live_tooltip(),
            HoverTarget::Tailscale => self.tailscale.tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, InternetMessage>> {
        let down = self.down.get()?;
        let handle_reading = source_reading(config, Some(down as &dyn BarWidget), 1.0)?;
        let handle = button(hoverable(
            handle_reading,
            HoverTarget::InternetDown,
            InternetMessage::Tooltip,
        ))
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(InternetMessage::Menu(MenuMessage::Toggle))
        .into();

        let reveal = self.menu.reveal();
        let mut pills: Vec<Element<'a, InternetMessage>> = Vec::new();
        if let Some(up) = self.up.get()
            && let Some(reading) = source_reading(config, Some(up as &dyn BarWidget), reveal)
        {
            pills.push(hoverable(
                reading,
                HoverTarget::InternetUp,
                InternetMessage::Tooltip,
            ));
        }
        if let Some(wireguard) = self.wireguard.get() {
            for tunnel in &wireguard.tunnels {
                if let Some(reading) =
                    source_reading(config, Some(tunnel as &dyn BarWidget), reveal)
                {
                    let name = tunnel.name.clone();
                    let hover = hoverable(
                        reading,
                        HoverTarget::Wireguard(name.clone()),
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
            let hover = hoverable(reading, HoverTarget::Tailscale, InternetMessage::Tooltip);
            pills.push(
                button(hover)
                    .padding(0)
                    .style(flat_button(config.palette.bar_foreground))
                    .on_press(InternetMessage::OpenTailscale)
                    .into(),
            );
        }

        let content = Row::with_children(pills)
            .spacing(config.spacing.widget_spacing_px)
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
