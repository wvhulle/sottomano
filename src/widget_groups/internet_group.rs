use iced::{Subscription, Task};

pub mod network_throughput_gauge;
pub mod tailscale_mesh_tooltip;
pub mod wireguard_tunnel_tooltip;

use crate::{
    config::Config,
    data::source::{
        Source, command,
        tailscale::{self, Tailscale, open_admin},
        throughput::{self, Download, Readings, Throughput, Total, Upload},
        wireguard::{TunnelAddress, Wireguard, disconnect},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{polled, polling},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::{HoverFetched, OnHoverReading},
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternetTarget {
    Down,
    Up,
    Total,
    Tailscale,
    Wireguard(String),
}

impl From<InternetTarget> for HoverTarget {
    fn from(target: InternetTarget) -> Self {
        HoverTarget::Internet(target)
    }
}

#[derive(Default)]
pub struct InternetGroup {
    down: ContinuousReading<Throughput<Download>>,
    up: ContinuousReading<Throughput<Upload>>,
    total: ContinuousReading<Throughput<Total>>,
    wireguard: ContinuousReading<Wireguard>,
    tailscale: ContinuousReading<Tailscale>,
    installed: Installed,
    tunnel_address: OnHoverReading<TunnelAddress>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Installed {
    wireguard: bool,
    tailscale: bool,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Installed(Installed),
    Throughput(Readings),
    Wireguard(Result<Wireguard, command::Error>),
    TunnelAddressFetched(HoverFetched<TunnelAddress>),
    Tailscale(Result<Tailscale, tailscale::Error>),
    Expansion(ExpansionMessage),
    Disconnect(String),
    OpenTailscale,
}

impl InternetGroup {
    pub fn probe() -> Task<InternetMessage> {
        Task::future(async {
            InternetMessage::Installed(Installed {
                wireguard: Wireguard::available(&()).await,
                tailscale: Tailscale::available(&()).await,
            })
        })
    }

    pub fn subscription(&self) -> Subscription<InternetMessage> {
        let wireguard = if self.installed.wireguard {
            polling::<Wireguard>(()).map(InternetMessage::Wireguard)
        } else {
            Subscription::none()
        };
        let tailscale = if self.installed.tailscale {
            polling::<Tailscale>(()).map(InternetMessage::Tailscale)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            throughput::subscription().map(InternetMessage::Throughput),
            self.expansion.while_expanded(wireguard),
            self.expansion.while_expanded(tailscale),
        ])
    }

    pub fn update(&mut self, msg: InternetMessage) -> Task<InternetMessage> {
        use crate::message_subscriptions::blocking;
        match msg {
            InternetMessage::Installed(installed) => self.installed = installed,
            InternetMessage::Throughput(readings) => {
                self.down.load(Some(readings.down));
                self.up.load(Some(readings.up));
                self.total.load(Some(readings.total));
            }
            InternetMessage::Wireguard(reading) => self.wireguard.receive(reading),
            InternetMessage::TunnelAddressFetched(fetched) => self.tunnel_address.receive(fetched),
            InternetMessage::Tailscale(reading) => self.tailscale.receive(reading),
            InternetMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            InternetMessage::Disconnect(name) => {
                return blocking("wireguard down", move || disconnect(&name)).discard();
            }
            InternetMessage::OpenTailscale => {
                return blocking("tailscale admin", open_admin).discard();
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<InternetMessage> {
        self.tunnel_address.stop();
        let Some(HoverTarget::Internet(InternetTarget::Wireguard(tunnel))) = hovered else {
            return Task::none();
        };
        self.tunnel_address.fetch(
            polled::<TunnelAddress>(tunnel.clone()),
            InternetMessage::TunnelAddressFetched,
            config,
        )
    }

    pub fn tooltip_for(&self, target: &InternetTarget) -> Option<Tooltip> {
        match target {
            InternetTarget::Wireguard(name) => {
                let tunnel = self
                    .wireguard
                    .get()?
                    .tunnels
                    .iter()
                    .find(|t| &t.name == name)?;
                Some(Tooltip::of(tunnel.tooltip().with_sections(&[
                    self.tunnel_address.section(TunnelAddress::sections),
                ])))
            }
            InternetTarget::Down => self.down.live_tooltip(),
            InternetTarget::Up => self.up.live_tooltip(),
            InternetTarget::Total => self.total.live_tooltip(),
            InternetTarget::Tailscale => self.tailscale.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, InternetMessage> {
        let tunnels = self
            .wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels);
        GroupContent::new(
            &self.expansion,
            InternetMessage::Expansion,
            [
                self.down.group_item(InternetTarget::Down),
                self.up.group_item(InternetTarget::Up),
            ]
            .into_iter()
            .flatten()
            .chain(tunnels.iter().map(|tunnel| {
                GroupItem::reading(tunnel, InternetTarget::Wireguard(tunnel.name.clone()))
                    .on_right_press(
                        "disconnect the tunnel",
                        InternetMessage::Disconnect(tunnel.name.clone()),
                    )
            }))
            .chain(self.tailscale.display().map(|tailscale| {
                GroupItem::reading(tailscale, InternetTarget::Tailscale)
                    .on_press("open Tailscale", InternetMessage::OpenTailscale)
            })),
        )
        .face(HandleFace::Icon(
            Phosphor::ArrowsDownUp(Weight::Regular).into(),
        ))
        .preview(
            self.total
                .display()
                .map(|widget| PreviewedValue::Hoverable {
                    widget,
                    target: InternetTarget::Total.into(),
                }),
        )
    }
}
