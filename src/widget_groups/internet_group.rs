use iced::{Subscription, Task};

mod network_throughput_gauge;
mod tailscale_mesh_tooltip;
mod wireguard_tunnel_tooltip;

use crate::{
    config::Config,
    data::source::{
        Delivery,
        tailscale::{Tailscale, open_admin},
        throughput::{Download, Readings, Throughput, Total, Upload},
        wireguard::{TunnelAddress, Wireguard, disconnect},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, polling},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
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
    tunnel_address: OnHoverReading<TunnelAddress>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Throughput(Delivery<Readings>),
    Wireguard(Delivery<Wireguard>),
    TunnelAddressFetched(Delivery<TunnelAddress>),
    Tailscale(Delivery<Tailscale>),
    Expansion(ExpansionMessage),
    Disconnect(String),
    OpenTailscale,
}

impl InternetGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, forced: Option<ForcedRefresh>) -> Subscription<InternetMessage> {
        Subscription::batch([
            polling::<Readings>((), forced).map(InternetMessage::Throughput),
            self.expansion
                .while_expanded(polling::<Wireguard>((), forced).map(InternetMessage::Wireguard)),
            self.expansion
                .while_expanded(polling::<Tailscale>((), forced).map(InternetMessage::Tailscale)),
        ])
    }

    pub fn update(&mut self, msg: InternetMessage) -> Task<InternetMessage> {
        use crate::message_subscriptions::blocking;
        match msg {
            InternetMessage::Throughput(Delivery::Reading(Ok(readings))) => {
                self.down.receive(Delivery::Reading(Ok(readings.down)));
                self.up.receive(Delivery::Reading(Ok(readings.up)));
                self.total.receive(Delivery::Reading(Ok(readings.total)));
            }
            InternetMessage::Throughput(Delivery::Reading(Err(never))) => match never {},
            InternetMessage::Throughput(Delivery::Absent) => {
                self.down.receive(Delivery::Absent);
                self.up.receive(Delivery::Absent);
                self.total.receive(Delivery::Absent);
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
        self.tunnel_address.poll(
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
            InternetTarget::Down => self.down.tooltip(),
            InternetTarget::Up => self.up.tooltip(),
            InternetTarget::Total => self.total.tooltip(),
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
