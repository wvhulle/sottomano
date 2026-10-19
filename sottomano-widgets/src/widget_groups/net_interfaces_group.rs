use iced::{Subscription, Task};

mod bluetooth_devices_tooltip;
mod ethernet_link_tooltip;
mod interface_sections;
mod mobile_modem_tooltip;
mod wifi_link_tooltip;

use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    network_connectivity::{
        bluez_bluetooth::Bluetooth,
        ethernet_connection::{Ethernet, EthernetDetails},
        modem_manager_mobile::Mobile,
        wifi_connection::{Wifi, WifiDetails},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::source_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
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

    pub fn update(
        &mut self,
        msg: NetInterfacesMessage,
        clock: Clock,
    ) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Bluetooth(delivery) => {
                self.bluetooth.receive_delivery(delivery, clock);
            }
            NetInterfacesMessage::Ethernet(delivery) => {
                self.ethernet.receive_delivery(delivery, clock);
            }
            NetInterfacesMessage::Wifi(delivery) => self.wifi.receive_delivery(delivery, clock),
            NetInterfacesMessage::Mobile(delivery) => {
                self.mobile.receive_delivery(*delivery, clock);
            }
            NetInterfacesMessage::WifiDetailsFetched(fetched) => {
                self.wifi_details.receive_delivery(*fetched, clock);
            }
            NetInterfacesMessage::EthernetDetailsFetched(fetched) => {
                self.ethernet_details.receive_delivery(fetched, clock);
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
        clock: Clock,
    ) -> Task<NetInterfacesMessage> {
        self.wifi_details.stop();
        self.ethernet_details.stop();
        match hovered {
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Wifi)) => {
                self.wifi_details.start_hover_fetch(
                    buses.clone(),
                    |fetched| NetInterfacesMessage::WifiDetailsFetched(Box::new(fetched)),
                    config,
                    clock,
                )
            }
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Ethernet)) => {
                self.ethernet_details.start_hover_fetch(
                    buses.clone(),
                    NetInterfacesMessage::EthernetDetailsFetched,
                    config,
                    clock,
                )
            }
            _ => Task::none(),
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: NetInterfaceTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            NetInterfaceTarget::Bluetooth => self.bluetooth.tooltip(clock),
            NetInterfaceTarget::Ethernet => self
                .ethernet
                .tooltip_with_sections(self.ethernet_details.sections(clock), clock),
            NetInterfaceTarget::Wifi => self
                .wifi
                .tooltip_with_sections(self.wifi_details.sections(clock), clock),
            NetInterfaceTarget::Mobile => self.mobile.tooltip(clock),
        }
    }

    pub fn group_content(&self, clock: Clock) -> GroupContent<'_, NetInterfacesMessage> {
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
        .preview(self.preferred_uplink(clock).map(PreviewedValue::Plain))
    }

    fn preferred_uplink(&self, clock: Clock) -> Option<&dyn BarWidget> {
        [self.mobile.display(), self.wifi.display()]
            .into_iter()
            .flatten()
            .find(|widget| widget.state(clock) != WidgetState::Off && widget.gauge().is_some())
    }
}
