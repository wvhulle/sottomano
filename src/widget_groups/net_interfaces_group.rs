use iced::{Subscription, Task};

mod bluetooth_devices_tooltip;
mod ethernet_link_tooltip;
mod interface_sections;
mod mobile_modem_tooltip;
mod wifi_link_tooltip;

use crate::{
    config::Config,
    data::source::{
        Delivery,
        net_interfaces::{
            bluetooth::Bluetooth,
            ethernet::{Ethernet, EthernetDetails},
            mobile::Mobile,
            wifi::{Wifi, WifiDetails},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::listening,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        on_hover_reading::OnHoverReading,
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
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Bluetooth(Delivery<Bluetooth>),
    Ethernet(Delivery<Ethernet>),
    Wifi(Delivery<Wifi>),
    Mobile(Box<Delivery<Mobile>>),
    WifiDetailsFetched(Box<Delivery<WifiDetails>>),
    EthernetDetailsFetched(Delivery<EthernetDetails>),
    Expansion(ExpansionMessage),
}

impl NetInterfacesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self) -> Subscription<NetInterfacesMessage> {
        Subscription::batch([
            self.expansion
                .while_expanded(listening::<Bluetooth>(()).map(NetInterfacesMessage::Bluetooth)),
            self.expansion
                .while_expanded(listening::<Ethernet>(()).map(NetInterfacesMessage::Ethernet)),
            listening::<Wifi>(()).map(NetInterfacesMessage::Wifi),
            listening::<Mobile>(())
                .map(|delivery| NetInterfacesMessage::Mobile(Box::new(delivery))),
        ])
    }

    pub fn update(&mut self, msg: NetInterfacesMessage) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(delivery) => self.bluetooth.push(delivery),
            NetInterfacesMessage::Ethernet(delivery) => self.ethernet.push(delivery),
            NetInterfacesMessage::Wifi(delivery) => self.wifi.push(delivery),
            NetInterfacesMessage::Mobile(delivery) => self.mobile.push(*delivery),
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
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Wifi)) => self.wifi_details.listen(
                (),
                |fetched| NetInterfacesMessage::WifiDetailsFetched(Box::new(fetched)),
                config,
            ),
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Ethernet)) => self
                .ethernet_details
                .listen((), NetInterfacesMessage::EthernetDetailsFetched, config),
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
