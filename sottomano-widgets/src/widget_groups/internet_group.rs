use iced::{Subscription, Task};

mod network_throughput_gauge;

use sottomano_sources::{
    SnapshotDelivery, network_connectivity::interface_throughput::InterfaceThroughput,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternetTarget {
    Down,
    Up,
    Throughput,
}

impl From<InternetTarget> for HoverTarget {
    fn from(target: InternetTarget) -> Self {
        HoverTarget::Internet(target)
    }
}

#[derive(Default)]
pub struct InternetGroup {
    throughput: LatestSnapshot<InterfaceThroughput>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum InternetMessage {
    Throughput(SnapshotDelivery<InterfaceThroughput>),
    Expansion(ExpansionMessage),
}

impl InternetGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, buses: &DesktopBuses) -> Subscription<InternetMessage> {
        snapshot_subscription::<InterfaceThroughput>(buses.clone()).map(InternetMessage::Throughput)
    }

    pub fn refresh(buses: &DesktopBuses) -> Task<InternetMessage> {
        first_delivery::<InterfaceThroughput>(buses.clone()).map(InternetMessage::Throughput)
    }

    pub fn update(&mut self, msg: InternetMessage, clock: Clock) {
        match msg {
            InternetMessage::Throughput(delivery) => {
                self.throughput.receive(delivery, clock);
            }
            InternetMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &InternetTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            InternetTarget::Down => self
                .throughput
                .tooltip_with(self.throughput.get()?.down.tooltip(clock)),
            InternetTarget::Up => self
                .throughput
                .tooltip_with(self.throughput.get()?.up.tooltip(clock)),
            InternetTarget::Throughput => self.throughput.tooltip(clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, InternetMessage> {
        let throughput = self.throughput.get();
        GroupContent::new(
            &self.expansion,
            InternetMessage::Expansion,
            self.throughput
                .group_item(InternetTarget::Throughput)
                .into_iter()
                .chain(throughput.into_iter().flat_map(|readings| {
                    [
                        GroupItem::reading(&readings.down, InternetTarget::Down)
                            .previewed_on_handle(),
                        GroupItem::reading(&readings.up, InternetTarget::Up),
                    ]
                })),
        )
        .face(HandleFace::Icon {
            glyph: Phosphor::ArrowsDownUp(Weight::Regular).into(),
            handle: GroupHandle::Internet,
        })
    }
}
