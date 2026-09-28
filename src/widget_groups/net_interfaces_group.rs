use iced::{Element, Subscription, Task, widget::Row};

pub mod bluetooth_devices_tooltip;
pub mod ethernet_link_tooltip;
pub mod interface_sections;
pub mod mobile_modem_tooltip;
pub mod wifi_link_tooltip;

use crate::{
    config::Config,
    data::source::net_interfaces::{
        bluetooth::{self, Bluetooth},
        ethernet::{self, Ethernet},
        mobile::{self, Mobile},
        wifi::{self, Wifi},
    },
    message_subscriptions::dbus,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        bar_widget::BarWidget,
        last_poll::LastPoll,
        menu_group::{MenuGroup, MenuMessage},
        phosphor,
        reading_row::{Reading, ReadingRow},
        tooltip_body::Tooltip,
    },
};

#[derive(Default)]
pub struct NetInterfacesGroup {
    bluetooth: LastPoll<Bluetooth>,
    ethernet: LastPoll<Ethernet>,
    wifi: LastPoll<Wifi>,
    mobile: LastPoll<Mobile>,
    pub(crate) menu: MenuGroup,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(Option<Bluetooth>),
    Ethernet(Option<Ethernet>),
    Wifi(Option<Wifi>),
    Mobile(Option<Mobile>),
    Menu(MenuMessage),
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

    pub fn update(&mut self, msg: NetInterfacesMessage) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(reading) => self.bluetooth.load(reading),
            NetInterfacesMessage::Ethernet(reading) => self.ethernet.load(reading),
            NetInterfacesMessage::Wifi(reading) => self.wifi.load(reading),
            NetInterfacesMessage::Mobile(reading) => self.mobile.load(reading),
            NetInterfacesMessage::Menu(msg) => {
                self.menu.update(msg);
            }
            NetInterfacesMessage::Tooltip(_) => {}
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

    pub fn view<'a>(
        &'a self,
        config: &'a Config,
        position: Position,
    ) -> Option<Element<'a, NetInterfacesMessage>> {
        let readings = move |reveal: f32| {
            ReadingRow::new()
                .opacity(reveal)
                .push(Reading::new(
                    self.bluetooth.display(),
                    HoverTarget::Bluetooth,
                ))
                .push(Reading::new(self.ethernet.display(), HoverTarget::Ethernet))
                .push(Reading::new(self.wifi.display(), HoverTarget::Wifi))
                .push(Reading::new(self.mobile.display(), HoverTarget::Mobile))
                .row(config, NetInterfacesMessage::Tooltip)
        };
        readings(1.0)?;

        let content = move |reveal: f32| readings(reveal).unwrap_or_else(Row::new).into();
        let slot = self.menu.slot(
            move || Self::face(config),
            content,
            NetInterfacesMessage::Menu,
        );
        let slot = match self.preferred_uplink() {
            Some(widget) => slot.previewed_value(widget),
            None => slot,
        };
        Some(slot.view(config, position))
    }

    fn preferred_uplink(&self) -> Option<&dyn BarWidget> {
        [self.mobile.display(), self.wifi.display()]
            .into_iter()
            .flatten()
            .find(|widget| !widget.disabled() && widget.value().is_some())
    }

    fn face<'a>(config: &Config) -> Element<'a, NetInterfacesMessage> {
        phosphor::Link::Regular.view(config.font.icon_size_px, config.palette.bar_foreground)
    }
}
