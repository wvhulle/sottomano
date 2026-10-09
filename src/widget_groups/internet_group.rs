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
        wireguard::{Wireguard, disconnect},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        bar_widget::BarWidget,
        group_item::{GroupItem, Press},
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, PreviewedValue, WidgetGroup,
        },
    },
};

#[derive(Default)]
pub struct InternetGroup {
    down: LastPoll<Throughput<Download>>,
    up: LastPoll<Throughput<Upload>>,
    total: LastPoll<Throughput<Total>>,
    wireguard: LastPoll<Wireguard>,
    tailscale: LastPoll<Tailscale>,
    installed: Installed,
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
            wireguard,
            tailscale,
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
            HoverTarget::InternetTotal => self.total.live_tooltip(),
            HoverTarget::Tailscale => self.tailscale.tooltip(),
            _ => None,
        }
    }

    pub fn widget_group<'a>(
        &'a self,
        config: &'a Config,
    ) -> Option<WidgetGroup<'a, InternetMessage>> {
        let tunnels = self
            .wireguard
            .get()
            .map_or(&[][..], |wireguard| &wireguard.tunnels);
        let tunnel_items = tunnels.iter().map(|tunnel| {
            Some(GroupItem {
                on_right_press: Some(Press {
                    does: "disconnect the tunnel",
                    message: InternetMessage::Disconnect(tunnel.name.clone()),
                }),
                ..GroupItem::reading(tunnel, HoverTarget::Wireguard(tunnel.name.clone()))
            })
        });
        let tailscale_item = self.tailscale.display().map(|tailscale| GroupItem {
            on_press: Some(Press {
                does: "open Tailscale",
                message: InternetMessage::OpenTailscale,
            }),
            ..GroupItem::reading(tailscale, HoverTarget::Tailscale)
        });
        let items: Vec<Option<GroupItem<'a, InternetMessage>>> = [
            self.down.reading_item(HoverTarget::InternetDown),
            self.up.reading_item(HoverTarget::InternetUp),
        ]
        .into_iter()
        .chain(tunnel_items)
        .chain([tailscale_item])
        .collect();
        WidgetGroup::new(
            &self.expansion,
            InternetMessage::Expansion,
            config,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::ArrowsDownUp(Weight::Regular).into()),
                preview: self
                    .total
                    .display()
                    .map(|widget| PreviewedValue::Hoverable {
                        widget,
                        target: HoverTarget::InternetTotal,
                    }),
                forwards_scroll: false,
            },
            &items,
        )
    }
}
