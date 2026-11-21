use std::{
    fmt::Debug,
    future::{Future, ready},
};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::sink_write_errors::GuestPowerError;
use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::GroupIndicator, indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub enum RowPower<PowerRequest> {
    SwitchedOn(PowerRequest),
    SwitchedOff(PowerRequest),
    Stoppable(PowerRequest),
}

pub struct FamilyIndicator<'a, Family: ContainerFamily> {
    pub indicator: GroupIndicator<'a>,
    pub power: RowPower<Family::PowerRequest>,
}

pub trait ContainerFamily: Debug + Clone + Sized + 'static {
    type Probe: SnapshotSource<StreamInputs = ()>;
    type Listings: SnapshotSource<StreamInputs = ()>;
    type Detail: SnapshotSource<StreamInputs = ()>;
    type PowerRequest: Debug + Clone + Send + 'static;

    const FAMILY: IndicatorGroupFamily;
    const GROUP_NAME: &'static str;
    const STOP_HINT: &'static str;
    const START_HINT: &'static str = "start";

    fn group_glyph() -> IconGlyph;

    fn running_count(probe: &Self::Probe) -> usize;

    fn listing_indicators(listings: &Self::Listings) -> Vec<FamilyIndicator<'_, Self>>;

    fn detail_indicators(detail: &Self::Detail) -> Vec<FamilyIndicator<'_, Self>>;

    fn stopped_indicators(_listings: &Self::Listings) -> Vec<FamilyIndicator<'_, Self>> {
        Vec::new()
    }

    fn stop(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), GuestPowerError>> + Send + 'static;

    fn start(
        request: Self::PowerRequest,
    ) -> impl Future<Output = Result<(), GuestPowerError>> + Send + 'static {
        ready(Err(GuestPowerError::StartUnsupported {
            group_name: Self::GROUP_NAME,
            request: format!("{request:?}"),
        }))
    }
}

pub struct ContainerFamilyGroup<Family: ContainerFamily> {
    probe: SubscribedSnapshot<Family::Probe>,
    listings: SubscribedSnapshot<Family::Listings>,
    detail: SubscribedSnapshot<Family::Detail>,
    expansion: IndicatorGroupExpansion,
}

impl<Family: ContainerFamily> Default for ContainerFamilyGroup<Family> {
    fn default() -> Self {
        Self {
            probe: SubscribedSnapshot::default(),
            listings: SubscribedSnapshot::default(),
            detail: SubscribedSnapshot::default(),
            expansion: IndicatorGroupExpansion::default(),
        }
    }
}

#[derive(Debug)]
pub enum ContainerFamilyMessage<Family: ContainerFamily> {
    Probe(SnapshotDelivery<Family::Probe>),
    Listings(SnapshotDelivery<Family::Listings>),
    Detail(SnapshotDelivery<Family::Detail>),
    PowerChangeRefused(GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum ContainerFamilyInteraction<Family: ContainerFamily> {
    Stop(Family::PowerRequest),
    Start(Family::PowerRequest),
    Expansion(IndicatorGroupExpansionMessage),
}

impl<Family: ContainerFamily> ContainerFamilyGroup<Family>
where
    ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
{
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        &self,
        demand: GroupSourceDemand,
    ) -> Subscription<ContainerFamilyMessage<Family>> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch([
            snapshot_subscription::<Family::Probe>(()).map(ContainerFamilyMessage::Probe),
            demand
                .while_headline_shown(snapshot_subscription::<Family::Listings>(()))
                .map(ContainerFamilyMessage::Listings),
            demand
                .while_indicators_shown(snapshot_subscription::<Family::Detail>(()))
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
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: ContainerFamilyInteraction<Family>,
    ) -> Task<ContainerFamilyMessage<Family>> {
        match interaction {
            ContainerFamilyInteraction::<Family>::Stop(request) => {
                refused_power_change(Family::stop(request))
            }
            ContainerFamilyInteraction::<Family>::Start(request) => {
                refused_power_change(Family::start(request))
            }
            ContainerFamilyInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn detail_indicators(&self) -> Vec<FamilyIndicator<'_, Family>> {
        self.detail
            .get()
            .map(|detail| Family::detail_indicators(detail))
            .unwrap_or_default()
    }

    fn listing_indicators(&self) -> Vec<FamilyIndicator<'_, Family>> {
        self.listings
            .get()
            .map(|listings| Family::listing_indicators(listings))
            .unwrap_or_default()
    }

    fn stopped_indicators(&self) -> Vec<FamilyIndicator<'_, Family>> {
        self.listings
            .get()
            .map(|listings| Family::stopped_indicators(listings))
            .unwrap_or_default()
    }

    fn group_rows(&self) -> Vec<GroupIndicator<'_>> {
        let detail_indicators = self.detail_indicators();
        let mut group_rows: Vec<GroupIndicator<'_>> = if detail_indicators.is_empty() {
            self.listing_indicators()
                .into_iter()
                .map(|row| switchable_indicator(stamped_by_source(&self.listings, row)))
                .collect()
        } else {
            detail_indicators
                .into_iter()
                .map(|row| switchable_indicator(stamped_by_source(&self.detail, row)))
                .collect()
        };
        group_rows.extend(
            self.stopped_indicators()
                .into_iter()
                .map(|row| switchable_indicator(stamped_by_source(&self.listings, row))),
        );
        group_rows
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            Family::FAMILY,
            GroupIndicator::glyph_only(
                Family::FAMILY.title(),
                Family::group_glyph(),
                match self.probe.get() {
                    Some(probe) if Family::running_count(probe) > 0 => IndicatorState::On,
                    Some(_) | None => IndicatorState::Off,
                },
            ),
            &self.expansion,
            ContainerFamilyInteraction::Expansion,
            self.group_rows(),
        )
    }
}

fn refused_power_change<Family: ContainerFamily>(
    power_change: impl Future<Output = Result<(), GuestPowerError>> + Send + 'static,
) -> Task<ContainerFamilyMessage<Family>> {
    Task::future(async move { power_change.await.err() })
        .and_then(|refusal| Task::done(ContainerFamilyMessage::PowerChangeRefused(refusal)))
}

fn stamped_by_source<'a, Family: ContainerFamily, Source: SnapshotSource>(
    source: &'a SubscribedSnapshot<Source>,
    row: FamilyIndicator<'a, Family>,
) -> FamilyIndicator<'a, Family> {
    FamilyIndicator {
        indicator: row.indicator.wrap_hover_card(|indicator_card| {
            Box::new(move |clock| source.hover_card_with(indicator_card(clock)))
        }),
        ..row
    }
}

fn switchable_indicator<Family: ContainerFamily>(
    row: FamilyIndicator<'_, Family>,
) -> GroupIndicator<'_>
where
    ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
{
    let indicator = row.indicator;
    match row.power {
        RowPower::SwitchedOn(request) => indicator.on_switch_press(
            SwitchPosition::On,
            Family::STOP_HINT,
            ContainerFamilyInteraction::<Family>::Stop(request),
        ),
        RowPower::SwitchedOff(request) => indicator.on_switch_press(
            SwitchPosition::Off,
            Family::START_HINT,
            ContainerFamilyInteraction::<Family>::Start(request),
        ),
        RowPower::Stoppable(request) => indicator.on_stop_press(
            Family::STOP_HINT,
            ContainerFamilyInteraction::<Family>::Stop(request),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use bytesize::ByteSize;
    use sottomano_desktop_sources::{
        SnapshotDelivery, SnapshotSource,
        container_family_snapshots::{FamilyDetails, FamilyListings},
        container_run_state::ContainerRunState,
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
        lxc_containers::LxcContainerListing,
        virtualbox_machines::{
            RegisteredVirtualBoxMachine, VirtualBoxFrontend, VirtualBoxMachine,
            VirtualBoxMachineListing, VirtualBoxMachineListings, VirtualBoxSettingsFile,
        },
        vmware_machines::{VmwareGuestOs, VmwareMachine, VmwareMachineListing},
    };

    use super::{ContainerFamily, ContainerFamilyGroup, ContainerFamilyInteraction, RowPower};
    use crate::{
        clock::Clock,
        hover_cards::{HoverTarget, IndicatorKey, hover_card_body::Section},
        indicator_groups::{
            indicator_group_identity::IndicatorGroupIdentity,
            indicator_group_interaction::IndicatorGroupInteraction,
            lxc_containers_group::LxcFamily,
            virtualbox_machines_group::{VirtualBoxMachinesGroup, VirtualBoxMachinesMessage},
            vmware_machines_group::{VmwareMachinesGroup, VmwareMachinesMessage},
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

    fn shown_card_sections<Family: ContainerFamily>(
        group: &ContainerFamilyGroup<Family>,
        row_name: &'static str,
        clock: Clock,
    ) -> Vec<Section>
    where
        ContainerFamilyInteraction<Family>: Into<IndicatorGroupInteraction>,
    {
        let hover_target = HoverTarget::Indicator(IndicatorKey {
            group: IndicatorGroupIdentity::sole(Family::FAMILY),
            name: row_name.into(),
        });
        group
            .group_content()
            .hover_card(&hover_target, clock)
            .map(|indicator_card| indicator_card.sections)
            .unwrap_or_default()
    }

    fn group_with_listing(clock: Clock) -> VmwareMachinesGroup {
        let mut group = VmwareMachinesGroup::default();
        let _ = group.update(
            VmwareMachinesMessage::Listings(polled(FamilyListings {
                listings: vec![VmwareMachineListing {
                    pid: LISTED_PID,
                    machine_name: "listed".to_owned(),
                    configuration_file: PathBuf::from("/vm/listed.vmx"),
                }],
            })),
            clock,
        );
        group
    }

    fn detailed_machines(clock: Clock) -> FamilyDetails<VmwareMachine> {
        FamilyDetails {
            details: vec![VmwareMachine {
                listing: VmwareMachineListing {
                    pid: DETAILED_PID,
                    machine_name: "detailed".to_owned(),
                    configuration_file: PathBuf::from("/vm/detailed.vmx"),
                },
                guest_os: VmwareGuestOs::Undeclared,
                guest_memory: GuestMemorySize::HypervisorDefault,
                guest_cores: GuestCoreCount::HypervisorDefault,
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
        assert!(!shown_card_sections(&group, "listed", clock).is_empty());
    }

    #[test]
    fn detail_replaces_listings_once_it_has_indicators() {
        let clock = Clock::real();
        let mut group = group_with_listing(clock);
        let _ = group.update(
            VmwareMachinesMessage::Detail(polled(detailed_machines(clock))),
            clock,
        );
        assert_eq!(group.group_rows().len(), 1);
        assert!(!shown_card_sections(&group, "detailed", clock).is_empty());
        assert!(shown_card_sections(&group, "listed", clock).is_empty());
    }

    #[test]
    fn running_containers_switch_off_and_stopped_ones_switch_on() {
        let listings = FamilyListings {
            listings: vec![
                LxcContainerListing {
                    container_name: "running".to_owned(),
                    run_state: ContainerRunState::Running,
                },
                LxcContainerListing {
                    container_name: "stopped".to_owned(),
                    run_state: ContainerRunState::Stopped,
                },
            ],
        };
        let presses: Vec<String> = LxcFamily::listing_indicators(&listings)
            .into_iter()
            .map(|row| match row.power {
                RowPower::SwitchedOn(container_name) => format!("switch off {container_name}"),
                RowPower::SwitchedOff(container_name) => format!("switch on {container_name}"),
                RowPower::Stoppable(container_name) => format!("stop {container_name}"),
            })
            .collect();
        assert_eq!(presses, vec!["switch off running", "switch on stopped"]);
    }

    #[test]
    fn stopped_machines_follow_the_detailed_running_ones() {
        let clock = Clock::real();
        let running_listing = VirtualBoxMachineListing {
            pid: DETAILED_PID,
            machine_name: "windows11".to_owned(),
            machine_reference: "windows11".to_owned(),
        };
        let mut group = VirtualBoxMachinesGroup::default();
        let _ = group.update(
            VirtualBoxMachinesMessage::Listings(polled(VirtualBoxMachineListings {
                listings: vec![running_listing.clone()],
                stopped_machines: vec![RegisteredVirtualBoxMachine {
                    machine_name: "ubuntu".to_owned(),
                    machine_uuid: "0b1c2d3e".to_owned(),
                }],
            })),
            clock,
        );
        let _ = group.update(
            VirtualBoxMachinesMessage::Detail(polled(FamilyDetails {
                details: vec![VirtualBoxMachine {
                    listing: running_listing,
                    frontend: VirtualBoxFrontend::Headless,
                    guest_memory: GuestMemorySize::HypervisorDefault,
                    guest_cores: GuestCoreCount::HypervisorDefault,
                    settings_file: VirtualBoxSettingsFile::NotInDefaultFolder,
                    started_at: clock.wall,
                    memory_in_use: ByteSize::gib(2),
                }],
            })),
            clock,
        );
        assert_eq!(group.group_rows().len(), 2);
        assert!(!shown_card_sections(&group, "ubuntu 0b1c2d3e", clock).is_empty());
    }
}
