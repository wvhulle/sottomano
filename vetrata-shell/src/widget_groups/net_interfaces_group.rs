use iced::{Subscription, Task};

mod bluetooth_devices_tooltip;
mod ethernet_link_tooltip;
mod interface_sections;
mod mobile_modem_tooltip;
mod wifi_link_tooltip;

use vetrata_config::Config;
use vetrata_sources::{
    SourceDelivery,
    bus::DesktopBuses,
    net_interfaces::{
        bluetooth::Bluetooth,
        ethernet::{Ethernet, EthernetDetails},
        mobile::Mobile,
        wifi::{Wifi, WifiDetails},
    },
};

use crate::{
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::source_subscription,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::{ReadingSections, Tooltip},
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
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(SourceDelivery<Bluetooth>),
    Ethernet(SourceDelivery<Ethernet>),
    Wifi(SourceDelivery<Wifi>),
    Mobile(Box<SourceDelivery<Mobile>>),
    WifiDetailsFetched(Box<SourceDelivery<WifiDetails>>),
    EthernetDetailsFetched(SourceDelivery<EthernetDetails>),
    Expansion(ExpansionMessage),
}

impl NetInterfacesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<NetInterfacesMessage> {
        Subscription::batch([
            self.expansion.while_expanded(
                source_subscription::<Bluetooth>(buses.clone())
                    .map(NetInterfacesMessage::Bluetooth),
            ),
            self.expansion.while_expanded(
                source_subscription::<Ethernet>(buses.clone()).map(NetInterfacesMessage::Ethernet),
            ),
            source_subscription::<Wifi>(buses.clone()).map(NetInterfacesMessage::Wifi),
            source_subscription::<Mobile>(buses.clone())
                .map(|delivery| NetInterfacesMessage::Mobile(Box::new(delivery))),
        ])
    }

    pub fn update(&mut self, msg: NetInterfacesMessage) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(delivery) => self.bluetooth.receive_delivery(delivery),
            NetInterfacesMessage::Ethernet(delivery) => self.ethernet.receive_delivery(delivery),
            NetInterfacesMessage::Wifi(delivery) => self.wifi.receive_delivery(delivery),
            NetInterfacesMessage::Mobile(delivery) => self.mobile.receive_delivery(*delivery),
            NetInterfacesMessage::WifiDetailsFetched(fetched) => {
                self.wifi_details.receive_delivery(*fetched);
            }
            NetInterfacesMessage::EthernetDetailsFetched(fetched) => {
                self.ethernet_details.receive_delivery(fetched);
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
        buses: &DesktopBuses,
    ) -> Task<NetInterfacesMessage> {
        self.wifi_details.stop();
        self.ethernet_details.stop();
        match hovered {
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Wifi)) => {
                self.wifi_details.start_hover_fetch(
                    buses.clone(),
                    |fetched| NetInterfacesMessage::WifiDetailsFetched(Box::new(fetched)),
                    config,
                )
            }
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Ethernet)) => {
                self.ethernet_details.start_hover_fetch(
                    buses.clone(),
                    NetInterfacesMessage::EthernetDetailsFetched,
                    config,
                )
            }
            _ => Task::none(),
        }
    }

    pub fn tooltip_for(&self, target: NetInterfaceTarget) -> Option<Tooltip> {
        match target {
            NetInterfaceTarget::Bluetooth => self.bluetooth.tooltip(),
            NetInterfaceTarget::Ethernet => self
                .ethernet
                .tooltip_with_sections(self.ethernet_details.sections(EthernetDetails::sections)),
            NetInterfaceTarget::Wifi => self
                .wifi
                .tooltip_with_sections(self.wifi_details.sections(WifiDetails::sections)),
            NetInterfaceTarget::Mobile => self.mobile.tooltip(),
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
        .face(HandleFace::Icon {
            glyph: Phosphor::Link(Weight::Regular).into(),
            handle: GroupHandle::NetInterfaces,
        })
        .preview(self.preferred_uplink().map(PreviewedValue::Plain))
    }

    fn preferred_uplink(&self) -> Option<&dyn BarWidget> {
        [self.mobile.display(), self.wifi.display()]
            .into_iter()
            .flatten()
            .find(|widget| widget.state() != WidgetState::Off && widget.gauge().is_some())
    }
}
