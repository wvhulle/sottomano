use iced::{Subscription, Task};

pub mod bluetooth_devices_tooltip;
pub mod ethernet_link_tooltip;
pub mod interface_sections;
pub mod mobile_modem_tooltip;
pub mod wifi_link_tooltip;

use crate::{
    config::Config,
    data::source::net_interfaces::{
        bluetooth::{self, Bluetooth},
        ethernet::{self, Ethernet, EthernetDetails},
        mobile::{self, Mobile},
        wifi::{self, Wifi, WifiDetails},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::dbus,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        on_hover_reading::{HoverFetched, OnHoverReading},
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetInterfaceTarget {
    Bluetooth,
    Ethernet,
    Wifi,
    Mobile,
}

impl From<NetInterfaceTarget> for HoverTarget {
    fn from(target: NetInterfaceTarget) -> Self {
        HoverTarget::NetInterface(target)
    }
}

#[derive(Default)]
pub struct NetInterfacesGroup {
    bluetooth: ContinuousReading<Bluetooth>,
    ethernet: ContinuousReading<Ethernet>,
    wifi: ContinuousReading<Wifi>,
    mobile: ContinuousReading<Mobile>,
    hovered: Option<NetInterfaceTarget>,
    wifi_details: OnHoverReading<WifiDetails>,
    ethernet_details: OnHoverReading<EthernetDetails>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(Option<Bluetooth>),
    Ethernet(Option<Ethernet>),
    Wifi(Option<Wifi>),
    Mobile(Option<Mobile>),
    WifiDetailsFetched(HoverFetched<WifiDetails>),
    EthernetDetailsFetched(HoverFetched<EthernetDetails>),
    Expansion(ExpansionMessage),
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

    pub fn update(&mut self, msg: NetInterfacesMessage) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(reading) => self.bluetooth.load(reading),
            NetInterfacesMessage::Ethernet(reading) => self.ethernet.load(reading),
            NetInterfacesMessage::Wifi(reading) => self.wifi.load(reading),
            NetInterfacesMessage::Mobile(reading) => self.mobile.load(reading),
            NetInterfacesMessage::WifiDetailsFetched(fetched) => self.wifi_details.receive(fetched),
            NetInterfacesMessage::EthernetDetailsFetched(fetched) => {
                self.ethernet_details.receive(fetched);
            }
            NetInterfacesMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
    }

    pub fn hover(
        &mut self,
        target: Option<NetInterfaceTarget>,
        config: &Config,
    ) -> Task<NetInterfacesMessage> {
        if self.hovered == target {
            return Task::none();
        }
        self.hovered = target;
        self.wifi_details.stop();
        self.ethernet_details.stop();
        match target {
            Some(NetInterfaceTarget::Wifi) => self.wifi_details.fetch(
                wifi::details(),
                NetInterfacesMessage::WifiDetailsFetched,
                config,
            ),
            Some(NetInterfaceTarget::Ethernet) => self.ethernet_details.fetch(
                ethernet::details(),
                NetInterfacesMessage::EthernetDetailsFetched,
                config,
            ),
            Some(NetInterfaceTarget::Bluetooth | NetInterfaceTarget::Mobile) | None => Task::none(),
        }
    }

    pub fn tooltip_for(&self, target: NetInterfaceTarget) -> Option<Tooltip> {
        match target {
            NetInterfaceTarget::Bluetooth => self.bluetooth.live_tooltip(),
            NetInterfaceTarget::Ethernet => self.ethernet.live_tooltip_with_sections(&[self
                .ethernet_details
                .section(EthernetDetails::sections)]),
            NetInterfaceTarget::Wifi => self
                .wifi
                .live_tooltip_with_sections(&[self.wifi_details.section(WifiDetails::sections)]),
            NetInterfaceTarget::Mobile => self.mobile.live_tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, NetInterfacesMessage> {
        GroupContent::new(
            &self.expansion,
            NetInterfacesMessage::Expansion,
            [
                self.bluetooth.group_item(NetInterfaceTarget::Bluetooth),
                self.ethernet.group_item(NetInterfaceTarget::Ethernet),
                self.wifi.group_item(NetInterfaceTarget::Wifi),
                self.mobile.group_item(NetInterfaceTarget::Mobile),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon(Phosphor::Link(Weight::Regular).into()))
        .preview(self.preferred_uplink().map(PreviewedValue::Plain))
    }

    fn preferred_uplink(&self) -> Option<&dyn BarWidget> {
        [self.mobile.display(), self.wifi.display()]
            .into_iter()
            .flatten()
            .find(|widget| widget.state() != WidgetState::Off && widget.gauge().is_some())
    }
}
