//! odin's `NetworkInterfaces`: a flat row of per-radio indicators (bluetooth,
//! ethernet, wifi, mobile broadband), each hidden while its device is absent —
//! a machine with only wifi shows one pill. Read-only, like the journal group:
//! each indicator is hoverable for its own tooltip; odin wires no click here.

use iced::{Alignment, Element, Subscription, widget::Row};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{
        BarWidget, Tooltip,
        net_interfaces::{
            bluetooth::{self, Bluetooth},
            ethernet::{self, Ethernet},
            mobile::{self, Mobile},
            wifi::{self, Wifi},
        },
    },
    wayland_layers::tooltip::{HoverTarget, TooltipMessage, hoverable},
    widget_groups::source_cell::SourceCell,
    widgets::icon_gauge::source_reading,
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
            crate::message_subscriptions::dbus(bluetooth::NAME, bluetooth::run)
                .map(NetInterfacesMessage::Bluetooth),
            crate::message_subscriptions::dbus(ethernet::NAME, ethernet::run)
                .map(NetInterfacesMessage::Ethernet),
            crate::message_subscriptions::dbus(wifi::NAME, wifi::run)
                .map(NetInterfacesMessage::Wifi),
            crate::message_subscriptions::dbus(mobile::NAME, mobile::run)
                .map(NetInterfacesMessage::Mobile),
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

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, NetInterfacesMessage>> {
        // Ethernet, wifi, bluetooth, mobile — mirroring odin's declaration order.
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
        Some(
            Row::with_children(indicators)
                .spacing(config.layout.horizontal_widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        )
    }
}

/// One radio's indicator (icon + optional gauge + badge), hoverable for its
/// tooltip; `None` when that radio is absent.
fn indicator<'a>(
    config: &ResolvedConfig,
    widget: Option<&dyn BarWidget>,
    target: HoverTarget,
) -> Option<Element<'a, NetInterfacesMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    Some(hoverable(reading, target, NetInterfacesMessage::Tooltip))
}
