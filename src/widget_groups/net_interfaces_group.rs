use iced::{Alignment, Element, Subscription, widget::Row};

use crate::{
    config::Config,
    data::source::{
        BarWidget, Tooltip,
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
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widget_groups::source_cell::SourceCell,
    widgets::{icon_gauge::source_reading, pager_slot::permanently_inactive_slot},
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
        let indicators: Vec<Element<'a, NetInterfacesMessage>> = [
            indicator(
                config,
                self.bluetooth.get().map(|w| w as &dyn BarWidget),
                HoverTarget::Bluetooth,
            ),
            indicator(
                config,
                self.ethernet.get().map(|w| w as &dyn BarWidget),
                HoverTarget::Ethernet,
            ),
            indicator(
                config,
                self.wifi.get().map(|w| w as &dyn BarWidget),
                HoverTarget::Wifi,
            ),
            indicator(
                config,
                self.mobile.get().map(|w| w as &dyn BarWidget),
                HoverTarget::Mobile,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        if indicators.is_empty() {
            return None;
        }
        Some(permanently_inactive_slot(
            Row::with_children(indicators)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
            config,
            position,
        ))
    }
}

fn indicator<'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
    target: HoverTarget,
) -> Option<Element<'a, NetInterfacesMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    Some(hoverable(reading, target, NetInterfacesMessage::Tooltip))
}
