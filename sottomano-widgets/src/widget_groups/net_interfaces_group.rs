use iced::{Subscription, Task};

mod ethernet_link_tooltip;
mod interface_sections;
mod mobile_modem_tooltip;
mod wifi_link_tooltip;

use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::{Ethernet, EthernetDetails, toggle_ethernet_ports},
        modem_manager_mobile::{Mobile, ModemState, toggle_mobile_radio},
        wifi_connection::{Wifi, WifiDetails, toggle_wifi_radio},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetInterfaceTarget {
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
    ethernet: LatestSnapshot<Ethernet>,
    wifi: LatestSnapshot<Wifi>,
    mobile: LatestSnapshot<Mobile>,
    wifi_details: HoverFetchedSnapshot<WifiDetails>,
    ethernet_details: HoverFetchedSnapshot<EthernetDetails>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Ethernet(SnapshotDelivery<Ethernet>),
    Wifi(SnapshotDelivery<Wifi>),
    Mobile(Box<SnapshotDelivery<Mobile>>),
    WifiDetailsFetched(Box<SnapshotDelivery<WifiDetails>>),
    EthernetDetailsFetched(SnapshotDelivery<EthernetDetails>),
    ToggleRadio(NetInterfaceTarget),
    Expansion(ExpansionMessage),
}

impl NetInterfacesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<NetInterfacesMessage> {
        Subscription::batch([
            self.expansion.while_expanded(
                snapshot_subscription::<Ethernet>(buses.clone())
                    .map(NetInterfacesMessage::Ethernet),
            ),
            snapshot_subscription::<Wifi>(buses.clone()).map(NetInterfacesMessage::Wifi),
            snapshot_subscription::<Mobile>(buses.clone())
                .map(|delivery| NetInterfacesMessage::Mobile(Box::new(delivery))),
        ])
    }

    pub fn update(
        &mut self,
        msg: NetInterfacesMessage,
        clock: Clock,
    ) -> Task<NetInterfacesMessage> {
        match msg {
            NetInterfacesMessage::Ethernet(delivery) => {
                self.ethernet.receive(delivery, clock);
            }
            NetInterfacesMessage::Wifi(delivery) => self.wifi.receive(delivery, clock),
            NetInterfacesMessage::Mobile(delivery) => {
                self.mobile.receive(*delivery, clock);
            }
            NetInterfacesMessage::WifiDetailsFetched(fetched) => {
                self.wifi_details.receive(*fetched, clock);
            }
            NetInterfacesMessage::EthernetDetailsFetched(fetched) => {
                self.ethernet_details.receive(fetched, clock);
            }
            NetInterfacesMessage::ToggleRadio(target) => {
                return Task::future(async move {
                    if let Err(bus_error) = toggle_radio(target).await {
                        tracing::warn!("toggle {target:?}: {bus_error}");
                    }
                })
                .discard();
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
            NetInterfaceTarget::Ethernet => self
                .ethernet
                .tooltip_with_groups(self.ethernet_details.sections(clock), clock),
            NetInterfaceTarget::Wifi => self
                .wifi
                .tooltip_with_groups(self.wifi_details.sections(clock), clock),
            NetInterfaceTarget::Mobile => self.mobile.tooltip(clock),
        }
    }

    fn radio_on(&self, target: NetInterfaceTarget) -> bool {
        match target {
            NetInterfaceTarget::Ethernet => self
                .ethernet
                .get()
                .is_some_and(|ethernet| ethernet.connected_count() > 0),
            NetInterfaceTarget::Wifi => self.wifi.get().is_some_and(|wifi| wifi.radio_enabled),
            NetInterfaceTarget::Mobile => self
                .mobile
                .get()
                .and_then(|mobile| mobile.state)
                .is_some_and(|state| {
                    !matches!(state, ModemState::Disabled | ModemState::Disabling)
                }),
        }
    }

    fn radio_item<'group>(
        &'group self,
        radio: Option<&'group dyn BarWidget>,
        target: NetInterfaceTarget,
    ) -> Option<GroupItem<'group, NetInterfacesMessage>> {
        Some(GroupItem::reading(radio?, target).on_press(
            self.press_does(target),
            NetInterfacesMessage::ToggleRadio(target),
        ))
    }

    fn press_does(&self, target: NetInterfaceTarget) -> &'static str {
        let radio_on = self.radio_on(target);
        match target {
            NetInterfaceTarget::Ethernet if radio_on => "disconnect Ethernet",
            NetInterfaceTarget::Ethernet => "connect Ethernet",
            NetInterfaceTarget::Wifi if radio_on => "turn Wi-Fi off",
            NetInterfaceTarget::Wifi => "turn Wi-Fi on",
            NetInterfaceTarget::Mobile if radio_on => "turn mobile data off",
            NetInterfaceTarget::Mobile => "turn mobile data on",
        }
    }

    pub fn group_content(&self, clock: Clock) -> GroupContent<'_, NetInterfacesMessage> {
        let mobile_is_uplink = self
            .mobile
            .display()
            .is_some_and(|mobile| shows_uplink_signal(mobile, clock));
        let wifi_is_uplink = !mobile_is_uplink
            && self
                .wifi
                .display()
                .is_some_and(|wifi| shows_uplink_signal(wifi, clock));
        GroupContent::new(
            &self.expansion,
            NetInterfacesMessage::Expansion,
            [
                self.radio_item(self.ethernet.display(), NetInterfaceTarget::Ethernet),
                self.radio_item(self.wifi.display(), NetInterfaceTarget::Wifi)
                    .map(|item| item.previewed_on_handle_if(wifi_is_uplink)),
                self.radio_item(self.mobile.display(), NetInterfaceTarget::Mobile)
                    .map(|item| item.previewed_on_handle_if(mobile_is_uplink)),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::Link(Weight::Regular).into(),
            handle: GroupHandle::NetInterfaces,
        })
    }
}

fn shows_uplink_signal(widget: &dyn BarWidget, clock: Clock) -> bool {
    widget.state(clock) != WidgetState::Off && widget.gauge().is_some()
}

async fn toggle_radio(target: NetInterfaceTarget) -> zbus::Result<()> {
    match target {
        NetInterfaceTarget::Ethernet => toggle_ethernet_ports().await,
        NetInterfaceTarget::Wifi => toggle_wifi_radio().await,
        NetInterfaceTarget::Mobile => toggle_mobile_radio().await,
    }
}
