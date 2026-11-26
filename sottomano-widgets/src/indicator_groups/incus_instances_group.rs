mod incus_instance_indicators;

use std::collections::BTreeMap;

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    incus_instances::change_instance_state, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::RunChange,
    incus_instances::{DaemonInstance, DaemonInstanceListing, InstanceDaemon, InstanceDaemonProbe},
};

use self::incus_instance_indicators::{
    IncusInstanceIndicator, IncusInstanceListingIndicator, UnreadableDaemonIndicator,
    instance_power_switch,
};
use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        snapshot_absence::{AbsenceCause, awaiting_source_indicator},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const WATCHED_DAEMONS: [InstanceDaemon; 2] = [InstanceDaemon::Incus, InstanceDaemon::Lxd];

#[derive(Default)]
pub struct IncusInstancesGroup {
    probes: BTreeMap<InstanceDaemon, SubscribedSnapshot<InstanceDaemonProbe>>,
    listings: BTreeMap<InstanceDaemon, SubscribedSnapshot<FamilyListings<DaemonInstanceListing>>>,
    details: BTreeMap<InstanceDaemon, SubscribedSnapshot<FamilyDetails<DaemonInstance>>>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum IncusInstancesMessage {
    Probe(InstanceDaemon, SnapshotDelivery<InstanceDaemonProbe>),
    Listings(
        InstanceDaemon,
        SnapshotDelivery<FamilyListings<DaemonInstanceListing>>,
    ),
    Instances(
        InstanceDaemon,
        SnapshotDelivery<FamilyDetails<DaemonInstance>>,
    ),
    StateChangeRefused(GuestPowerError),
}

#[derive(Debug, Clone)]
pub enum IncusInstancesInteraction {
    ChangeState {
        instance_daemon: InstanceDaemon,
        project_name: String,
        instance_name: String,
        run_change: RunChange,
    },
    Expansion(IndicatorGroupExpansionMessage),
}

impl IncusInstancesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<IncusInstancesMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        Subscription::batch(WATCHED_DAEMONS.into_iter().flat_map(|instance_daemon| {
            [
                snapshot_subscription::<InstanceDaemonProbe>(instance_daemon)
                    .with(instance_daemon)
                    .map(|(instance_daemon, delivery)| {
                        IncusInstancesMessage::Probe(instance_daemon, delivery)
                    }),
                demand.while_headline_shown(
                    snapshot_subscription::<FamilyListings<DaemonInstanceListing>>(instance_daemon)
                        .with(instance_daemon)
                        .map(|(instance_daemon, delivery)| {
                            IncusInstancesMessage::Listings(instance_daemon, delivery)
                        }),
                ),
                demand.while_indicators_shown(
                    snapshot_subscription::<FamilyDetails<DaemonInstance>>(instance_daemon)
                        .with(instance_daemon)
                        .map(|(instance_daemon, delivery)| {
                            IncusInstancesMessage::Instances(instance_daemon, delivery)
                        }),
                ),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: IncusInstancesMessage,
        clock: Clock,
    ) -> Task<IncusInstancesMessage> {
        match message {
            IncusInstancesMessage::Probe(instance_daemon, delivery) => {
                self.probes
                    .entry(instance_daemon)
                    .or_default()
                    .receive(delivery, clock);
                Task::none()
            }
            IncusInstancesMessage::Listings(instance_daemon, delivery) => {
                self.listings
                    .entry(instance_daemon)
                    .or_default()
                    .receive(delivery, clock);
                Task::none()
            }
            IncusInstancesMessage::Instances(instance_daemon, delivery) => {
                self.details
                    .entry(instance_daemon)
                    .or_default()
                    .receive(delivery, clock);
                Task::none()
            }
            IncusInstancesMessage::StateChangeRefused(refusal) => {
                tracing::warn!("incus instance state: {refusal}");
                Task::none()
            }
        }
    }

    pub fn interact(
        &mut self,
        interaction: IncusInstancesInteraction,
    ) -> Task<IncusInstancesMessage> {
        match interaction {
            IncusInstancesInteraction::ChangeState {
                instance_daemon,
                project_name,
                instance_name,
                run_change,
            } => Task::future(async move {
                change_instance_state(instance_daemon, &project_name, &instance_name, run_change)
                    .await
                    .err()
            })
            .and_then(|refusal| Task::done(IncusInstancesMessage::StateChangeRefused(refusal))),
            IncusInstancesInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn probe_availability(&self, instance_daemon: InstanceDaemon) -> Option<Availability> {
        self.probes.get(&instance_daemon)?.get().map(
            |InstanceDaemonProbe {
                 daemon_availability: availability,
             }| *availability,
        )
    }

    pub fn group_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::IncusInstances,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::IncusInstances.title(),
                incus_glyph(),
                if WATCHED_DAEMONS.into_iter().any(|instance_daemon| {
                    self.probe_availability(instance_daemon) == Some(Availability::Available)
                }) {
                    IndicatorState::On
                } else {
                    IndicatorState::Off
                },
            ),
            &self.expansion,
            IncusInstancesInteraction::Expansion,
            WATCHED_DAEMONS
                .into_iter()
                .map(|instance_daemon| self.daemon_indicator_listing(instance_daemon))
                .collect(),
        )
    }

    fn daemon_indicator_listing(&self, instance_daemon: InstanceDaemon) -> IndicatorListing<'_> {
        if let Some(detail_indicator) = self.details.get(&instance_daemon)
            && let Some(instances) = detail_indicator.get()
        {
            return IndicatorListing::Listed(
                instances
                    .details
                    .iter()
                    .map(|instance| {
                        let row = IncusInstanceIndicator {
                            instance_daemon,
                            instance,
                        }
                        .indicator();
                        instance_power_switch(row, instance_daemon, &instance.listing)
                            .with_hover_card(Box::new(move |_| {
                                detail_indicator.hover_card_with(
                                    IncusInstanceIndicator {
                                        instance_daemon,
                                        instance,
                                    }
                                    .hover_card(),
                                )
                            }))
                    })
                    .collect(),
            );
        }
        if self.probe_availability(instance_daemon) == Some(Availability::Unavailable) {
            return IndicatorListing::Listed(Vec::new());
        }
        match self.listings.get(&instance_daemon) {
            Some(SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::FetchFailed(read_failure),
            }) => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![
                    UnreadableDaemonIndicator {
                        instance_daemon,
                        read_failure,
                    }
                    .indicator(),
                ],
            },
            Some(listing_indicator) => listing_indicator.indicator_listing(
                |listings| {
                    listings
                        .listings
                        .iter()
                        .map(|listing| {
                            let row = IncusInstanceListingIndicator {
                                instance_daemon,
                                listing,
                            }
                            .indicator();
                            instance_power_switch(row, instance_daemon, listing).with_hover_card(
                                Box::new(move |_| {
                                    listing_indicator.hover_card_with(
                                        IncusInstanceListingIndicator {
                                            instance_daemon,
                                            listing,
                                        }
                                        .hover_card(),
                                    )
                                }),
                            )
                        })
                        .collect()
                },
                incus_glyph(),
            ),
            None => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![awaiting_source_indicator::<
                    FamilyListings<DaemonInstanceListing>,
                >(incus_glyph())],
            },
        }
    }
}

fn incus_glyph() -> IconGlyph {
    Phosphor::CubeFocus(Weight::Regular).into()
}
