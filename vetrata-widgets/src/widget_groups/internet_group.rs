use iced::{Subscription, Task};

mod network_throughput_gauge;
mod tailscale_mesh_tooltip;
mod wireguard_tunnel_tooltip;

use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    bus::DesktopBuses,
    tailscale::{Tailscale, open_admin},
    throughput::Readings,
    wireguard::{TunnelAddress, Wireguard, disconnect},
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::{ReadingSections, Tooltip},
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
    throughput: ContinuousReading<Readings>,
    wireguard: ContinuousReading<Wireguard>,
    tailscale: ContinuousReading<Tailscale>,
    tunnel_address: OnHoverReading<TunnelAddress>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Throughput(SourceDelivery<Readings>),
    Wireguard(SourceDelivery<Wireguard>),
    TunnelAddressFetched(SourceDelivery<TunnelAddress>),
    Tailscale(SourceDelivery<Tailscale>),
    Expansion(ExpansionMessage),
    Disconnect(String),
    OpenTailscale,
}

impl InternetGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<InternetMessage> {
        Subscription::batch([
            source_subscription::<Readings>(buses.clone()).map(InternetMessage::Throughput),
            self.expansion.while_expanded(
                source_subscription::<Wireguard>(()).map(InternetMessage::Wireguard),
            ),
            self.expansion.while_expanded(
                source_subscription::<Tailscale>(()).map(InternetMessage::Tailscale),
            ),
        ])
    }

    pub fn refresh(target: &InternetTarget, buses: &DesktopBuses) -> Task<InternetMessage> {
        match target {
            InternetTarget::Down | InternetTarget::Up | InternetTarget::Total => {
                first_delivery::<Readings>(buses.clone()).map(InternetMessage::Throughput)
            }
            InternetTarget::Wireguard(_) => {
                first_delivery::<Wireguard>(()).map(InternetMessage::Wireguard)
            }
            InternetTarget::Tailscale => {
                first_delivery::<Tailscale>(()).map(InternetMessage::Tailscale)
            }
        }
    }

    pub fn update(&mut self, msg: InternetMessage) -> Task<InternetMessage> {
        use vetrata_sources::background_work::blocking;
        match msg {
            InternetMessage::Throughput(delivery) => {
                self.throughput.receive_delivery(delivery);
            }
            InternetMessage::Wireguard(reading) => self.wireguard.receive_delivery(reading),
            InternetMessage::TunnelAddressFetched(fetched) => {
                self.tunnel_address.receive_delivery(fetched);
            }
            InternetMessage::Tailscale(reading) => self.tailscale.receive_delivery(reading),
            InternetMessage::Expansion(m) => {
                self.expansion.update(m);
            }
            InternetMessage::Disconnect(name) => {
                return Task::future(blocking("wireguard down", move || disconnect(&name)))
                    .discard();
            }
            InternetMessage::OpenTailscale => {
                return Task::future(blocking("tailscale admin", open_admin)).discard();
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
        self.tunnel_address.start_hover_fetch(
            tunnel.clone(),
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
                Some(
                    self.wireguard
                        .tooltip_with(tunnel.tooltip())?
                        .with_sections(self.tunnel_address.sections(TunnelAddress::sections)),
                )
            }
            InternetTarget::Down => self
                .throughput
                .tooltip_with(self.throughput.get()?.down.tooltip()),
            InternetTarget::Up => self
                .throughput
                .tooltip_with(self.throughput.get()?.up.tooltip()),
            InternetTarget::Total => match self.throughput.get() {
                Some(readings) => self.throughput.tooltip_with(readings.total.tooltip()),
                None => self.throughput.tooltip(),
            },
            InternetTarget::Tailscale => self.tailscale.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, InternetMessage> {
        let tunnels = self
            .wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels);
        let throughput = self.throughput.get();
        GroupContent::new(
            &self.expansion,
            InternetMessage::Expansion,
            self.throughput
                .group_item(InternetTarget::Total)
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(&readings.down, InternetTarget::Down),
                        GroupItem::reading(&readings.up, InternetTarget::Up),
                    ]
                }))
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
        .face(HandleFace::Icon {
            glyph: Phosphor::ArrowsDownUp(Weight::Regular).into(),
            handle: GroupHandle::Internet,
        })
        .preview(throughput.map(|readings| PreviewedValue::Hoverable {
            widget: &readings.total,
            target: InternetTarget::Total.into(),
        }))
    }
}
