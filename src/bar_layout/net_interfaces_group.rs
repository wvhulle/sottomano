//! odin's `NetworkInterfaces`: a flat row of per-radio indicators (bluetooth,
//! ethernet, wifi, mobile broadband), each hidden while its device is absent —
//! a machine with only wifi shows one pill. Read-only, like the journal group:
//! each indicator is hoverable for its own tooltip; odin wires no click here.

use iced::{
    Alignment, Element, Subscription,
    widget::Row,
};

use crate::{
    bar_layout::source_cell::SourceCell,
    config::resolved::ResolvedConfig,
    data_sources::{
        BarWidget,
        net_interfaces::{bluetooth::Bluetooth, ethernet::Ethernet, mobile::Mobile, wifi::Wifi},
    },
    tooltip::{TooltipMessage, TooltipSource, hoverable},
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
            SourceCell::subscription().map(NetInterfacesMessage::Bluetooth),
            SourceCell::subscription().map(NetInterfacesMessage::Ethernet),
            SourceCell::subscription().map(NetInterfacesMessage::Wifi),
            SourceCell::subscription().map(NetInterfacesMessage::Mobile),
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

    pub fn bluetooth_cell(&self) -> &SourceCell<Bluetooth> {
        &self.bluetooth
    }

    pub fn ethernet_cell(&self) -> &SourceCell<Ethernet> {
        &self.ethernet
    }

    pub fn wifi_cell(&self) -> &SourceCell<Wifi> {
        &self.wifi
    }

    pub fn mobile_cell(&self) -> &SourceCell<Mobile> {
        &self.mobile
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Option<Element<'a, NetInterfacesMessage>> {
        // Ethernet, wifi, bluetooth, mobile — mirroring odin's declaration order.
        let indicators: Vec<Element<'a, NetInterfacesMessage>> = [
            indicator(
                config,
                self.bluetooth.get().map(|w| w as &dyn BarWidget),
                TooltipSource::Bluetooth,
            ),
            indicator(
                config,
                self.ethernet.get().map(|w| w as &dyn BarWidget),
                TooltipSource::Ethernet,
            ),
            indicator(
                config,
                self.wifi.get().map(|w| w as &dyn BarWidget),
                TooltipSource::Wifi,
            ),
            indicator(
                config,
                self.mobile.get().map(|w| w as &dyn BarWidget),
                TooltipSource::Mobile,
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
    source: TooltipSource,
) -> Option<Element<'a, NetInterfacesMessage>> {
    let reading = source_reading(config, widget, 1.0)?;
    Some(hoverable(reading, source, NetInterfacesMessage::Tooltip))
}
