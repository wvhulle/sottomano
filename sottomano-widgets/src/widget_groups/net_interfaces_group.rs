use iced::{Subscription, Task};

mod mobile_modem_tooltip;
mod wifi_link_tooltip;

use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    network_connectivity::{
        icmp_gateway_latency::GatewayLatencies,
        modem_manager_mobile::{Mobile, ModemState, toggle_mobile_radio},
        wifi_connection::{Wifi, WifiDetails, toggle_wifi_radio},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widget_groups::interface_address_sections::gateway_latency_sections,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        tooltip_body::tooltip_source_delivery::{SourcedSections, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetInterfaceTarget {
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
    wifi: LatestSnapshot<Wifi>,
    mobile: LatestSnapshot<Mobile>,
    wifi_details: HoverFetchedSnapshot<WifiDetails>,
    gateway_latencies: LatestSnapshot<GatewayLatencies, HidesFetchFailures>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum NetInterfacesMessage {
    Wifi(SnapshotDelivery<Wifi>),
    Mobile(Box<SnapshotDelivery<Mobile>>),
    WifiDetailsFetched(Box<SnapshotDelivery<WifiDetails>>),
    GatewayLatencies(SnapshotDelivery<GatewayLatencies>),
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
                snapshot_subscription::<GatewayLatencies>(buses.clone())
                    .map(NetInterfacesMessage::GatewayLatencies),
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
            NetInterfacesMessage::Wifi(delivery) => self.wifi.receive(delivery, clock),
            NetInterfacesMessage::Mobile(delivery) => {
                self.mobile.receive(*delivery, clock);
            }
            NetInterfacesMessage::WifiDetailsFetched(fetched) => {
                self.wifi_details.receive(*fetched, clock);
            }
            NetInterfacesMessage::GatewayLatencies(delivery) => {
                self.gateway_latencies.receive(delivery, clock);
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
        match hovered {
            Some(HoverTarget::NetInterface(NetInterfaceTarget::Wifi)) => {
                self.wifi_details.start_hover_fetch(
                    buses.clone(),
                    |fetched| NetInterfacesMessage::WifiDetailsFetched(Box::new(fetched)),
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
            NetInterfaceTarget::Wifi => {
                let wifi_gateway_sections = self
                    .wifi
                    .get()
                    .and_then(|wifi| self.gateway_sections(&wifi.device));
                self.wifi.tooltip_with_groups(
                    self.wifi_details
                        .sections(clock)
                        .into_iter()
                        .chain(wifi_gateway_sections),
                    clock,
                )
            }
            NetInterfaceTarget::Mobile => self.mobile.tooltip(clock),
        }
    }

    fn gateway_sections(&self, interface: &str) -> Option<SourcedSections> {
        self.gateway_latencies
            .sections_described_by(|latencies| gateway_latency_sections(latencies, interface))
    }

    fn radio_on(&self, target: NetInterfaceTarget) -> bool {
        match target {
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
        NetInterfaceTarget::Wifi => toggle_wifi_radio().await,
        NetInterfaceTarget::Mobile => toggle_mobile_radio().await,
    }
}
