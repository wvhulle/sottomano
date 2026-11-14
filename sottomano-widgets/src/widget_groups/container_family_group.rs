use std::{
    fmt::Debug,
    future::{Future, ready},
};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        value_cells::LabeledValue,
    },
};

pub enum RowPower<PowerRequest> {
    SwitchedOn(PowerRequest),
    SwitchedOff(PowerRequest),
    Stoppable(PowerRequest),
}

pub struct FamilyRow<Family: ContainerFamily, Reading> {
    pub target: Family::Target,
    pub reading: Reading,
    pub power: RowPower<Family::PowerRequest>,
}

pub trait ContainerFamily: Debug + Clone + Sized + 'static {
    type Probe: SnapshotSource<StreamInputs = ()>;
    type Listings: SnapshotSource<StreamInputs = ()>;
    type Detail: SnapshotSource<StreamInputs = ()>;
    type Target: Into<HoverTarget> + Clone + PartialEq;
    type PowerRequest: Debug + Clone + Send + 'static;
    type ListingRow<'a>: GroupItemReading + 'a
    where
        Self: 'a;
    type DetailRow<'a>: GroupItemReading + 'a
    where
        Self: 'a;

    const FAMILY: WidgetGroupFamily;
    const GROUP_NAME: &'static str;
    const STOP_HINT: &'static str;
    const START_HINT: &'static str = "start";

    fn group_glyph() -> IconGlyph;

    fn running_count(probe: &Self::Probe) -> usize;

    fn listing_rows(listings: &Self::Listings) -> Vec<FamilyRow<Self, Self::ListingRow<'_>>>;

    fn detail_rows(detail: &Self::Detail) -> Vec<FamilyRow<Self, Self::DetailRow<'_>>>;

    fn stop(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static;

    fn start(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), String>> + Send + 'static {
        ready(Err(format!(
            "{} cannot start {request:?} from the bar",
            Self::GROUP_NAME
        )))
    }

    fn group_label(probe: &Self::Probe) -> String {
        format!("{} running", Self::running_count(probe))
    }

    fn group_hover_entries(probe: &Self::Probe) -> Vec<LabeledValue> {
        vec![LabeledValue::new(
            "Running",
            Self::running_count(probe).to_string(),
        )]
    }
}

pub struct ContainerFamilyGroup<Family: ContainerFamily> {
    probe: LatestSnapshot<Family::Probe, HidesFetchFailures>,
    listings: LatestSnapshot<Family::Listings, HidesFetchFailures>,
    detail: LatestSnapshot<Family::Detail, HidesFetchFailures>,
    expansion: WidgetGroupExpansion,
}

impl<Family: ContainerFamily> Default for ContainerFamilyGroup<Family> {
    fn default() -> Self {
        Self {
            probe: LatestSnapshot::default(),
            listings: LatestSnapshot::default(),
            detail: LatestSnapshot::default(),
            expansion: WidgetGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum ContainerFamilyMessage<Family: ContainerFamily> {
    Probe(SnapshotDelivery<Family::Probe>),
    Listings(SnapshotDelivery<Family::Listings>),
    Detail(SnapshotDelivery<Family::Detail>),
    PowerChangeRefused(String),
    Interaction(ContainerFamilyInteraction<Family>),
}

#[derive(Debug, Clone)]
pub enum ContainerFamilyInteraction<Family: ContainerFamily> {
    Stop(Family::PowerRequest),
    Start(Family::PowerRequest),
    Expansion(WidgetGroupExpansionMessage),
}

impl<Family: ContainerFamily> ContainerFamilyGroup<Family> {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
    ) -> Subscription<ContainerFamilyMessage<Family>> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch([
            snapshot_subscription::<Family::Probe>(()).map(ContainerFamilyMessage::Probe),
            demand
                .while_face_shown(snapshot_subscription::<Family::Listings>(()))
                .map(ContainerFamilyMessage::Listings),
            demand
                .while_rows_shown(snapshot_subscription::<Family::Detail>(()))
                .map(ContainerFamilyMessage::Detail),
        ])
    }

    pub fn update(
        &mut self,
        message: ContainerFamilyMessage<Family>,
        clock: Clock,
    ) -> Task<ContainerFamilyMessage<Family>> {
        match message {
            ContainerFamilyMessage::Probe(delivery) => self.probe.receive(delivery, clock),
            ContainerFamilyMessage::Listings(delivery) => self.listings.receive(delivery, clock),
            ContainerFamilyMessage::Detail(delivery) => self.detail.receive(delivery, clock),
            ContainerFamilyMessage::PowerChangeRefused(refusal) => {
                tracing::warn!(group = Family::GROUP_NAME, %refusal, "power change refused");
            }
            ContainerFamilyMessage::Interaction(ContainerFamilyInteraction::Stop(request)) => {
                return refused_power_change(Family::stop(request));
            }
            ContainerFamilyMessage::Interaction(ContainerFamilyInteraction::Start(request)) => {
                return refused_power_change(Family::start(request));
            }
            ContainerFamilyMessage::Interaction(ContainerFamilyInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    fn detail_rows(&self) -> Vec<FamilyRow<Family, Family::DetailRow<'_>>> {
        self.detail
            .get()
            .map(Family::detail_rows)
            .unwrap_or_default()
    }

    fn listing_rows(&self) -> Vec<FamilyRow<Family, Family::ListingRow<'_>>> {
        self.listings
            .get()
            .map(Family::listing_rows)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &Family::Target, clock: Clock) -> Option<HoverCard> {
        let detail_rows = self.detail_rows();
        if !detail_rows.is_empty() {
            let row = detail_rows.into_iter().find(|row| row.target == *target)?;
            return self.detail.hover_card_with(row.reading.hover_card(clock));
        }
        let row = self
            .listing_rows()
            .into_iter()
            .find(|row| row.target == *target)?;
        self.listings.hover_card_with(row.reading.hover_card(clock))
    }

    fn group_rows(&self) -> Vec<GroupItem<'_, ContainerFamilyInteraction<Family>>> {
        let detail_rows = self.detail_rows();
        if !detail_rows.is_empty() {
            return detail_rows.into_iter().map(switchable_item).collect();
        }
        self.listing_rows()
            .into_iter()
            .map(switchable_item)
            .collect()
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, ContainerFamilyInteraction<Family>> {
        WidgetGroupContent::expandable(
            Family::FAMILY,
            &self.expansion,
            ContainerFamilyInteraction::Expansion,
            self.group_rows(),
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Family::group_glyph(),
            reading: self,
        })
    }
}

fn refused_power_change<Family: ContainerFamily>(
    power_change: impl Future<Output = Result<(), String>> + Send + 'static,
) -> Task<ContainerFamilyMessage<Family>> {
    Task::future(async move { power_change.await.err() })
        .and_then(|refusal| Task::done(ContainerFamilyMessage::PowerChangeRefused(refusal)))
}

fn switchable_item<'a, Family: ContainerFamily, Reading: GroupItemReading + 'a>(
    row: FamilyRow<Family, Reading>,
) -> GroupItem<'a, ContainerFamilyInteraction<Family>> {
    let item = GroupItem::reading(row.reading, row.target);
    match row.power {
        RowPower::SwitchedOn(request) => item.on_switch_press(
            SwitchPosition::On,
            Family::STOP_HINT,
            ContainerFamilyInteraction::Stop(request),
        ),
        RowPower::SwitchedOff(request) => item.on_switch_press(
            SwitchPosition::Off,
            Family::START_HINT,
            ContainerFamilyInteraction::Start(request),
        ),
        RowPower::Stoppable(request) => {
            item.on_stop_press(Family::STOP_HINT, ContainerFamilyInteraction::Stop(request))
        }
    }
}

impl<Family: ContainerFamily> GroupItemReading for ContainerFamilyGroup<Family> {
    fn name(&self) -> &'static str {
        Family::GROUP_NAME
    }

    fn icon(&self) -> IconGlyph {
        Family::group_glyph()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.probe.get().map(Family::group_label)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.probe.get() {
            Some(probe) if Family::running_count(probe) > 0 => ReadingDisplayState::On,
            Some(_) | None => ReadingDisplayState::Off,
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            self.probe
                .get()
                .map(Family::group_hover_entries)
                .unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use bytesize::ByteSize;
    use sottomano_desktop_sources::{
        SnapshotDelivery, SnapshotSource,
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        lxc_containers::{LxcContainerListing, LxcContainerListings},
        vmware_machines::{
            VmwareMachine, VmwareMachineListing, VmwareMachineListings, VmwareMachines,
        },
    };

    use super::{ContainerFamily, RowPower};
    use crate::{
        clock::Clock,
        widget_groups::{
            lxc_containers_group::LxcFamily,
            vmware_machines_group::{
                VmwareMachineTarget, VmwareMachinesGroup, VmwareMachinesMessage,
            },
        },
    };

    const LISTED_PID: i32 = 11;
    const DETAILED_PID: i32 = 22;

    fn polled<Source: SnapshotSource>(snapshot: Source) -> SnapshotDelivery<Source> {
        SnapshotDelivery::fixture_delivered(
            Ok(snapshot),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )
    }

    fn group_with_listing(clock: Clock) -> VmwareMachinesGroup {
        let mut group = VmwareMachinesGroup::default();
        let _ = group.update(
            VmwareMachinesMessage::Listings(polled(VmwareMachineListings {
                listings: vec![VmwareMachineListing {
                    pid: LISTED_PID,
                    machine_name: "listed".to_owned(),
                    configuration_file: PathBuf::from("/vm/listed.vmx"),
                    running: true,
                }],
            })),
            clock,
        );
        group
    }

    fn detailed_machines(clock: Clock) -> VmwareMachines {
        VmwareMachines {
            machines: vec![VmwareMachine {
                pid: DETAILED_PID,
                machine_name: "detailed".to_owned(),
                configuration_file: PathBuf::from("/vm/detailed.vmx"),
                guest_os: None,
                guest_memory: None,
                guest_cores: None,
                started_at: clock.wall,
                memory_in_use: ByteSize::mib(512),
            }],
        }
    }

    #[test]
    fn listings_answer_until_detail_arrives() {
        let clock = Clock::real();
        let group = group_with_listing(clock);
        assert_eq!(group.group_rows().len(), 1);
        assert!(
            group
                .hover_card_for(&VmwareMachineTarget::of_pid(LISTED_PID), clock)
                .is_some()
        );
    }

    #[test]
    fn detail_replaces_listings_once_it_has_rows() {
        let clock = Clock::real();
        let mut group = group_with_listing(clock);
        let _ = group.update(
            VmwareMachinesMessage::Detail(polled(detailed_machines(clock))),
            clock,
        );
        assert_eq!(group.group_rows().len(), 1);
        assert!(
            group
                .hover_card_for(&VmwareMachineTarget::of_pid(DETAILED_PID), clock)
                .is_some()
        );
        assert!(
            group
                .hover_card_for(&VmwareMachineTarget::of_pid(LISTED_PID), clock)
                .is_none()
        );
    }

    #[test]
    fn running_containers_switch_off_and_stopped_ones_switch_on() {
        let listings = LxcContainerListings {
            listings: vec![
                LxcContainerListing {
                    container_name: "running".to_owned(),
                    running: true,
                },
                LxcContainerListing {
                    container_name: "stopped".to_owned(),
                    running: false,
                },
            ],
        };
        let presses: Vec<String> = LxcFamily::listing_rows(&listings)
            .into_iter()
            .map(|row| match row.power {
                RowPower::SwitchedOn(container_name) => format!("switch off {container_name}"),
                RowPower::SwitchedOff(container_name) => format!("switch on {container_name}"),
                RowPower::Stoppable(container_name) => format!("stop {container_name}"),
            })
            .collect();
        assert_eq!(presses, vec!["switch off running", "switch on stopped"]);
    }
}
