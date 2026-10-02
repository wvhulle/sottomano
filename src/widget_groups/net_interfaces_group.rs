use iced::{Subscription, Task};

pub mod bluetooth_devices_tooltip;
pub mod ethernet_link_tooltip;
pub mod interface_sections;
pub mod mobile_modem_tooltip;
pub mod wifi_link_tooltip;

use crate::{
    data::source::net_interfaces::{
        bluetooth::{self, Bluetooth},
        ethernet::{self, Ethernet},
        mobile::{self, Mobile},
        wifi::{self, Wifi},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::dbus,
    widgets::{
        bar_widget::BarWidget,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
        widget_state::WidgetState,
    },
};

#[derive(Default)]
pub struct NetInterfacesGroup {
    bluetooth: LastPoll<Bluetooth>,
    ethernet: LastPoll<Ethernet>,
    wifi: LastPoll<Wifi>,
    mobile: LastPoll<Mobile>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(Option<Bluetooth>),
    Ethernet(Option<Ethernet>),
    Wifi(Option<Wifi>),
    Mobile(Option<Mobile>),
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
            NetInterfacesMessage::Expansion(msg) => {
                self.expansion.update(msg);
            }
        }
        Task::none()
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

    pub fn widget_group(&self) -> WidgetGroup<'_, NetInterfacesMessage> {
        WidgetGroup::new(&self.expansion, NetInterfacesMessage::Expansion)
            .icon(Phosphor::Link(Weight::Regular))
            .previewed_value(self.preferred_uplink())
            .reading(self.bluetooth.display(), HoverTarget::Bluetooth)
            .reading(self.ethernet.display(), HoverTarget::Ethernet)
            .reading(self.wifi.display(), HoverTarget::Wifi)
            .reading(self.mobile.display(), HoverTarget::Mobile)
    }

    fn preferred_uplink(&self) -> Option<&dyn BarWidget> {
        [self.mobile.display(), self.wifi.display()]
            .into_iter()
            .flatten()
            .find(|widget| widget.state() != WidgetState::Off && widget.gauge().is_some())
    }
}
