use iced::{Subscription, Task};

pub mod bluetooth_devices_tooltip;
pub mod ethernet_link_tooltip;
pub mod interface_sections;
pub mod mobile_modem_tooltip;
pub mod wifi_link_tooltip;

use crate::{
    config::Config,
    data::source::net_interfaces::{
        bluetooth::{self, Bluetooth, BluetoothPresence},
        ethernet::{self, Ethernet, EthernetDetails, EthernetPresence},
        mobile::{self, Mobile, ModemPresence},
        wifi::{self, Wifi, WifiDetails, WifiPresence},
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
    wifi_details: OnHoverReading<WifiDetails>,
    ethernet_details: OnHoverReading<EthernetDetails>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(BluetoothPresence),
    Ethernet(EthernetPresence),
    Wifi(WifiPresence),
    Mobile(ModemPresence),
    WifiDetailsFetched(Box<HoverFetched<WifiDetails>>),
    EthernetDetailsFetched(HoverFetched<EthernetDetails>),
    Expansion(ExpansionMessage),
}

impl NetInterfacesGroup {
    pub fn subscription(&self) -> Subscription<NetInterfacesMessage> {
        Subscription::batch([
            self.expansion.while_expanded(
                dbus(bluetooth::NAME, bluetooth::readings).map(NetInterfacesMessage::Bluetooth),
            ),
            self.expansion.while_expanded(
                dbus(ethernet::NAME, ethernet::readings).map(NetInterfacesMessage::Ethernet),
            ),
            dbus(wifi::NAME, wifi::readings).map(NetInterfacesMessage::Wifi),
            dbus(mobile::NAME, mobile::readings).map(NetInterfacesMessage::Mobile),
        ])
    }

    pub fn update(&mut self, msg: NetInterfacesMessage) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(presence) => match presence {
                BluetoothPresence::Adapter(bluetooth) => self.bluetooth.load(bluetooth),
                BluetoothPresence::NoAdapter => self.bluetooth = ContinuousReading::default(),
            },
            NetInterfacesMessage::Ethernet(presence) => match presence {
                EthernetPresence::Ports(ethernet) => self.ethernet.load(ethernet),
                EthernetPresence::NoPort => self.ethernet = ContinuousReading::default(),
            },
            NetInterfacesMessage::Wifi(presence) => match presence {
                WifiPresence::Adapter(wifi) => self.wifi.load(wifi),
                WifiPresence::NoAdapter => self.wifi = ContinuousReading::default(),
            },
            NetInterfacesMessage::Mobile(presence) => match presence {
                ModemPresence::Modem(mobile) => self.mobile.load(*mobile),
                ModemPresence::NoModem => self.mobile = ContinuousReading::default(),
            },
            NetInterfacesMessage::WifiDetailsFetched(fetched) => {
                self.wifi_details.receive(*fetched);
            }
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
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<NetInterfacesMessage> {
        self.wifi_details.stop();
        self.ethernet_details.stop();
        match hovered {
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Wifi)) => self.wifi_details.fetch(
                wifi::details(),
                |fetched| NetInterfacesMessage::WifiDetailsFetched(Box::new(fetched)),
                config,
            ),
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Ethernet)) => {
                self.ethernet_details.fetch(
                    ethernet::details(),
                    NetInterfacesMessage::EthernetDetailsFetched,
                    config,
                )
            }
            _ => Task::none(),
        }
    }

    pub fn tooltip_for(&self, target: NetInterfaceTarget) -> Option<Tooltip> {
        match target {
            NetInterfaceTarget::Bluetooth => self.bluetooth.live_tooltip(),
            NetInterfaceTarget::Ethernet => self.ethernet.live_tooltip_with_sections(
                &self.ethernet_details.sections(EthernetDetails::sections),
            ),
            NetInterfaceTarget::Wifi => self
                .wifi
                .live_tooltip_with_sections(&self.wifi_details.sections(WifiDetails::sections)),
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
