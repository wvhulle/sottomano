use iced::{
    Element, Subscription, Task,
    widget::{Row, button},
};

use crate::{
    config::Config,
    data::{
        source::{
            BarWidget, Tooltip, command,
            tailscale::{self, Tailscale, open_admin},
            throughput::{self, Download, Readings, Throughput, Upload},
            wireguard::{Wireguard, disconnect},
        },
        source_cell::SourceCell,
    },
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        flat_button,
        menu_group::{MenuGroup, MenuMessage},
        reading_row::{Reading, ReadingRow},
        source_reading::source_reading,
    },
};

#[derive(Default)]
pub struct InternetGroup {
    down: SourceCell<Throughput<Download>>,
    up: SourceCell<Throughput<Upload>>,
    wireguard: SourceCell<Wireguard>,
    tailscale: SourceCell<Tailscale>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Throughput(Readings),
    Wireguard(Result<Option<Wireguard>, command::Error>),
    Tailscale(Result<Option<Tailscale>, tailscale::Error>),
    Menu(MenuMessage),
    Disconnect(String),
    OpenTailscale,
    Tooltip(TooltipMessage),
}

impl InternetGroup {
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
            InternetMessage::Wireguard(reading) => self.wireguard.receive(reading),
            InternetMessage::Tailscale(reading) => self.tailscale.receive(reading),
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
        let tunnels = self
            .wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels);
        let content = tunnels
            .iter()
            .fold(
                ReadingRow::new()
                    .opacity(reveal)
                    .push(Reading::new(self.up.display(), HoverTarget::InternetUp)),
                |readings, tunnel| {
                    readings.push(
                        Reading::new(Some(tunnel), HoverTarget::Wireguard(tunnel.name.clone()))
                            .on_right_press(InternetMessage::Disconnect(tunnel.name.clone())),
                    )
                },
            )
            .push(
                Reading::new(self.tailscale.display(), HoverTarget::Tailscale)
                    .on_press(InternetMessage::OpenTailscale),
            )
            .row(config, InternetMessage::Tooltip)
            .unwrap_or_else(Row::new);

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
