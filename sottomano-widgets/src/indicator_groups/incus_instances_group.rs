use std::{borrow::Cow, collections::BTreeMap};

use iced::{Subscription, Task};
use sottomano_desktop_sinks::{
    incus_instances::change_instance_state, sink_write_errors::GuestPowerError,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::{ContainerRunState, RunChange},
    incus_instances::{
        DaemonInstance, DaemonInstanceListing, InstanceDaemon, InstanceDaemonProbe, InstanceKind,
    },
    podman_container_engine::libpod_running_containers::EngineReadError,
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
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        engine_container_indicators::engine_failure_label,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
        process_usage_cells::approximate_memory_text,
    },
    widgets::{subscribed_snapshot::SubscribedSnapshot, value_cells::LabeledValue},
};

const WATCHED_DAEMONS: [InstanceDaemon; 2] = [InstanceDaemon::Incus, InstanceDaemon::Lxd];
const START_HINT: &str = "start the instance";
const STOP_HINT: &str = "stop the instance";

fn kind_text(kind: InstanceKind) -> &'static str {
    match kind {
        InstanceKind::Container => "container",
        InstanceKind::VirtualMachine => "virtual machine",
    }
}

fn instance_icon(kind: InstanceKind) -> IconGlyph {
    match kind {
        InstanceKind::Container => Phosphor::Package(Weight::Regular).into(),
        InstanceKind::VirtualMachine => Phosphor::DesktopTower(Weight::Regular).into(),
    }
}

fn instance_power_switch<'a>(
    row: GroupIndicator<'a>,
    instance_daemon: InstanceDaemon,
    listing: &DaemonInstanceListing,
) -> GroupIndicator<'a> {
    let (switch_position, hint, run_change) = match listing.run_state {
        ContainerRunState::Running => (SwitchPosition::On, STOP_HINT, RunChange::Stop),
        ContainerRunState::Stopped => (SwitchPosition::Off, START_HINT, RunChange::Start),
    };
    row.on_switch_press(
        switch_position,
        hint,
        IncusInstancesInteraction::ChangeState {
            instance_daemon,
            project_name: listing.project_name.clone(),
            instance_name: listing.instance_name.clone(),
            run_change,
        },
    )
}

fn daemon_instance_name(
    instance_daemon: InstanceDaemon,
    project_name: &str,
    instance_name: &str,
) -> Cow<'static, str> {
    let in_project = match project_name {
        "default" => instance_name.to_owned(),
        _ => format!("{instance_name} in {project_name}"),
    };
    match instance_daemon {
        InstanceDaemon::Incus => in_project.into(),
        InstanceDaemon::Lxd => format!("{in_project} on LXD").into(),
    }
}

struct IncusInstanceIndicator<'a> {
    instance_daemon: InstanceDaemon,
    instance: &'a DaemonInstance,
}

impl<'a> IncusInstanceIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        let label = if self.instance.listing.run_state == ContainerRunState::Stopped {
            Some(self.instance.listing.status_text.to_lowercase())
        } else {
            self.instance
                .global_addresses
                .first()
                .cloned()
                .or_else(|| self.instance.memory_in_use.map(approximate_memory_text))
        };
        GroupIndicator::glyph_only(
            daemon_instance_name(
                self.instance_daemon,
                &self.instance.listing.project_name,
                &self.instance.listing.instance_name,
            ),
            instance_icon(self.instance.listing.kind),
            self.instance.listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: label.map_or(ReadoutLabel::Unlabeled, ReadoutLabel::Labeled),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        let instance = self.instance;
        let mut detail_entries = vec![
            LabeledValue::new("Kind", kind_text(instance.listing.kind)),
            LabeledValue::new("Status", instance.listing.status_text.clone()),
            LabeledValue::new("Daemon", self.instance_daemon.product_name()),
            LabeledValue::new("Project", instance.listing.project_name.clone()),
        ];
        if let Some(memory_in_use) = instance.memory_in_use {
            detail_entries.push(LabeledValue::new(
                "Memory use",
                approximate_memory_text(memory_in_use),
            ));
        }
        detail_entries.extend(
            instance
                .global_addresses
                .iter()
                .map(|global_address| LabeledValue::new("Address", global_address.clone())),
        );
        if let Some(image_description) = &instance.image_description {
            detail_entries.push(LabeledValue::new("Image", image_description.clone()));
        }
        vec![Section::new("Instance", dictionary_table(detail_entries))]
    }
}

struct IncusInstanceListingIndicator<'a> {
    instance_daemon: InstanceDaemon,
    listing: &'a DaemonInstanceListing,
}

impl<'a> IncusInstanceListingIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            daemon_instance_name(
                self.instance_daemon,
                &self.listing.project_name,
                &self.listing.instance_name,
            ),
            instance_icon(self.listing.kind),
            self.listing.run_state.into(),
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(self.listing.status_text.to_lowercase()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Instance",
            dictionary_table([
                LabeledValue::new("Kind", kind_text(self.listing.kind)),
                LabeledValue::new("Status", self.listing.status_text.clone()),
                LabeledValue::new("Daemon", self.instance_daemon.product_name()),
                LabeledValue::new("Project", self.listing.project_name.clone()),
            ]),
        )]
    }
}

struct UnreadableDaemonIndicator<'a> {
    instance_daemon: InstanceDaemon,
    read_failure: &'a EngineReadError,
}

impl<'a> UnreadableDaemonIndicator<'a> {
    fn indicator(self) -> GroupIndicator<'a> {
        GroupIndicator::glyph_only(
            self.instance_daemon.product_name(),
            Phosphor::Package(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Error",
            dictionary_table([
                LabeledValue::new("Problem", self.read_failure.to_string()),
                LabeledValue::new(
                    "Socket",
                    self.instance_daemon.socket_path().display().to_string(),
                ),
            ]),
        )]
    }
}

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
        let rows = WATCHED_DAEMONS
            .into_iter()
            .flat_map(|instance_daemon| self.daemon_indicators(instance_daemon));
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::IncusInstances,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::IncusInstances.title(),
                Phosphor::CubeFocus(Weight::Regular).into(),
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
            rows,
        )
    }

    fn daemon_indicators(&self, instance_daemon: InstanceDaemon) -> Vec<GroupIndicator<'_>> {
        if let Some(detail_indicator) = self.details.get(&instance_daemon)
            && let Some(instances) = detail_indicator.get()
        {
            return instances
                .details
                .iter()
                .map(|instance| {
                    let row = IncusInstanceIndicator {
                        instance_daemon,
                        instance,
                    }
                    .indicator();
                    instance_power_switch(row, instance_daemon, &instance.listing).with_hover_card(
                        Box::new(move |_| {
                            detail_indicator.hover_card_with(
                                IncusInstanceIndicator {
                                    instance_daemon,
                                    instance,
                                }
                                .hover_card(),
                            )
                        }),
                    )
                })
                .collect();
        }
        let Some(listing_indicator) = self.listings.get(&instance_daemon) else {
            return Vec::new();
        };
        if let Some(read_failure) = listing_indicator.fetch_failure() {
            return vec![
                UnreadableDaemonIndicator {
                    instance_daemon,
                    read_failure,
                }
                .indicator(),
            ];
        }
        listing_indicator
            .get()
            .map(|listings| listings.listings.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|listing| {
                let row = IncusInstanceListingIndicator {
                    instance_daemon,
                    listing,
                }
                .indicator();
                instance_power_switch(row, instance_daemon, listing).with_hover_card(Box::new(
                    move |_| {
                        listing_indicator.hover_card_with(
                            IncusInstanceListingIndicator {
                                instance_daemon,
                                listing,
                            }
                            .hover_card(),
                        )
                    },
                ))
            })
            .collect()
    }
}
