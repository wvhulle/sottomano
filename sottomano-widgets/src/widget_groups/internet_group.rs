use iced::{Subscription, Task};

mod network_throughput_gauge;
mod tailscale_mesh_tooltip;
mod wireguard_tunnel_tooltip;

use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    network_connectivity::{
        interface_throughput::InterfaceThroughput,
        tailscale_status::{self, Tailscale, open_admin},
        wireguard_tunnels::{TunnelAddress, TunnelOnBus, Wireguard, disconnect},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternetTarget {
    Down,
    Up,
    Throughput,
    Tailscale,
    Wireguard(String),
}

impl From<tailscale_status::Error> for Section {
    fn from(error: tailscale_status::Error) -> Self {
        Section::error(error)
    }
}

impl From<InternetTarget> for HoverTarget {
    fn from(target: InternetTarget) -> Self {
        HoverTarget::Internet(target)
    }
}

#[derive(Default)]
pub struct InternetGroup {
    throughput: ContinuousReading<InterfaceThroughput>,
    wireguard: ContinuousReading<Wireguard>,
    tailscale: ContinuousReading<Tailscale>,
    tunnel_address: OnHoverReading<TunnelAddress>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Throughput(SourceDelivery<InterfaceThroughput>),
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
            source_subscription::<InterfaceThroughput>(buses.clone())
                .map(InternetMessage::Throughput),
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
            InternetTarget::Down | InternetTarget::Up | InternetTarget::Throughput => {
                first_delivery::<InterfaceThroughput>(buses.clone())
                    .map(InternetMessage::Throughput)
            }
            InternetTarget::Wireguard(_) => {
                first_delivery::<Wireguard>(()).map(InternetMessage::Wireguard)
            }
            InternetTarget::Tailscale => {
                first_delivery::<Tailscale>(()).map(InternetMessage::Tailscale)
            }
        }
    }

    pub fn update(&mut self, msg: InternetMessage, clock: Clock) -> Task<InternetMessage> {
        use sottomano_sources::blocking_background_work::blocking;
        match msg {
            InternetMessage::Throughput(delivery) => {
                self.throughput.receive_delivery(delivery, clock);
            }
            InternetMessage::Wireguard(reading) => self.wireguard.receive_delivery(reading, clock),
            InternetMessage::TunnelAddressFetched(fetched) => {
                self.tunnel_address.receive_delivery(fetched, clock);
            }
            InternetMessage::Tailscale(reading) => self.tailscale.receive_delivery(reading, clock),
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
        buses: Option<&DesktopBuses>,
        clock: Clock,
    ) -> Task<InternetMessage> {
        self.tunnel_address.stop();
        let Some(HoverTarget::Internet(InternetTarget::Wireguard(tunnel))) = hovered else {
            return Task::none();
        };
        let Some(buses) = buses else {
            return Task::none();
        };
        self.tunnel_address.start_hover_fetch(
            TunnelOnBus {
                buses: buses.clone(),
                tunnel: tunnel.clone(),
            },
            InternetMessage::TunnelAddressFetched,
            config,
            clock,
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &InternetTarget, clock: Clock) -> Option<Tooltip> {
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
                        .tooltip_with(tunnel.tooltip(clock))?
                        .with_sections(self.tunnel_address.sections(clock)),
                )
            }
            InternetTarget::Down => self
                .throughput
                .tooltip_with(self.throughput.get()?.down.tooltip(clock)),
            InternetTarget::Up => self
                .throughput
                .tooltip_with(self.throughput.get()?.up.tooltip(clock)),
            InternetTarget::Throughput => self.throughput.tooltip(clock),
            InternetTarget::Tailscale => self.tailscale.tooltip(clock),
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
                .group_item(InternetTarget::Throughput)
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(&readings.down, InternetTarget::Down)
                            .previewed_on_handle(),
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
    }
}
