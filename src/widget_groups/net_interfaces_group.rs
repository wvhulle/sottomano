use iced::{Element, Subscription};

use crate::{
    config::Config,
    data::source::{
        Tooltip,
        net_interfaces::{
            bluetooth::{self, Bluetooth},
            ethernet::{self, Ethernet},
            mobile::{self, Mobile},
            wifi::{self, Wifi},
        },
    },
    message_subscriptions::dbus,
    wayland_layers::{
        surface::Position,
        tooltip::{CarriesTooltip, HoverTarget, TooltipMessage},
    },
    widget_groups::{pills::Pills, source_cell::SourceCell},
};

#[derive(Default)]
pub struct NetInterfacesGroup {
    bluetooth: SourceCell<Bluetooth>,
    ethernet: SourceCell<Ethernet>,
    wifi: SourceCell<Wifi>,
    mobile: SourceCell<Mobile>,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(Option<Bluetooth>),
    Ethernet(Option<Ethernet>),
    Wifi(Option<Wifi>),
    Mobile(Option<Mobile>),
    Tooltip(TooltipMessage),
}

impl CarriesTooltip for NetInterfacesMessage {
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

impl NetInterfacesGroup {
    pub fn subscription() -> Subscription<NetInterfacesMessage> {
        Subscription::batch([
            dbus(bluetooth::NAME, bluetooth::readings).map(NetInterfacesMessage::Bluetooth),
            dbus(ethernet::NAME, ethernet::readings).map(NetInterfacesMessage::Ethernet),
            dbus(wifi::NAME, wifi::readings).map(NetInterfacesMessage::Wifi),
            dbus(mobile::NAME, mobile::readings).map(NetInterfacesMessage::Mobile),
        ])
    }

    pub fn update(&mut self, msg: NetInterfacesMessage) {
        match msg {
            NetInterfacesMessage::Bluetooth(reading) => self.bluetooth.load(reading),
            NetInterfacesMessage::Ethernet(reading) => self.ethernet.load(reading),
            NetInterfacesMessage::Wifi(reading) => self.wifi.load(reading),
            NetInterfacesMessage::Mobile(reading) => self.mobile.load(reading),
            NetInterfacesMessage::Tooltip(_) => {}
        }
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Bluetooth => self.bluetooth.live_tooltip(),
            HoverTarget::Ethernet => self.ethernet.live_tooltip(),
            HoverTarget::Wifi => self.wifi.live_tooltip(),
            HoverTarget::Mobile => self.mobile.live_tooltip(),
            _ => None,
        }
    }

    pub fn view<'a>(
        &self,
        config: &Config,
        position: Position,
    ) -> Option<Element<'a, NetInterfacesMessage>> {
        Pills::new()
            .push(self.bluetooth.get(), HoverTarget::Bluetooth)
            .push(self.ethernet.get(), HoverTarget::Ethernet)
            .push(self.wifi.get(), HoverTarget::Wifi)
            .push(self.mobile.get(), HoverTarget::Mobile)
            .slot(config, position)
    }
}
