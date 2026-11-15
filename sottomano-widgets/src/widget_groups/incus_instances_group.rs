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
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
        toggle_control::SwitchPosition,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
    },
    widget_groups::{
        engine_container_rows::engine_failure_label,
        process_usage_cells::approximate_memory_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const WATCHED_DAEMONS: [InstanceDaemon; 2] = [InstanceDaemon::Incus, InstanceDaemon::Lxd];
const START_HINT: &str = "start the instance";
const STOP_HINT: &str = "stop the instance";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncusInstanceTarget {
    pub instance_daemon: InstanceDaemon,
    pub instance_name: Cow<'static, str>,
}

impl From<IncusInstanceTarget> for HoverTarget {
    fn from(target: IncusInstanceTarget) -> Self {
        HoverTarget::IncusInstance(target)
    }
}

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
    row: GroupItem<'a, IncusInstancesInteraction>,
    instance_daemon: InstanceDaemon,
    listing: &DaemonInstanceListing,
) -> GroupItem<'a, IncusInstancesInteraction> {
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

struct IncusInstanceRow<'a> {
    instance_daemon: InstanceDaemon,
    instance: &'a DaemonInstance,
}

impl<'a> IncusInstanceRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        let label = if self.instance.listing.run_state == ContainerRunState::Stopped {
            Some(self.instance.listing.status_text.to_lowercase())
        } else {
            self.instance
                .global_addresses
                .first()
                .cloned()
                .or_else(|| self.instance.memory_in_use.map(approximate_memory_text))
        };
        ReadingRow {
            readout: RowReadout {
                label: label.map_or(ReadoutLabel::Unlabeled, ReadoutLabel::Labeled),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                daemon_instance_name(
                    self.instance_daemon,
                    &self.instance.listing.project_name,
                    &self.instance.listing.instance_name,
                ),
                instance_icon(self.instance.listing.kind),
                self.instance.listing.run_state.into(),
                Box::new(move |_| self.hover_card()),
            )
        }
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

struct IncusInstanceListingRow<'a> {
    instance_daemon: InstanceDaemon,
    listing: &'a DaemonInstanceListing,
}

impl<'a> IncusInstanceListingRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(self.listing.status_text.to_lowercase()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                daemon_instance_name(
                    self.instance_daemon,
                    &self.listing.project_name,
                    &self.listing.instance_name,
                ),
                instance_icon(self.listing.kind),
                self.listing.run_state.into(),
                Box::new(move |_| self.hover_card()),
            )
        }
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

struct UnreadableDaemonRow<'a> {
    instance_daemon: InstanceDaemon,
    read_failure: &'a EngineReadError,
}

impl<'a> UnreadableDaemonRow<'a> {
    fn reading_row(self) -> ReadingRow<'a> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(engine_failure_label(self.read_failure).to_owned()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                self.instance_daemon.product_name(),
                Phosphor::Package(Weight::Regular).into(),
                ReadingDisplayState::Failed,
                Box::new(move |_| self.hover_card()),
            )
        }
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
    expansion: WidgetGroupExpansion,
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
    Interaction(IncusInstancesInteraction),
}

#[derive(Debug, Clone)]
pub enum IncusInstancesInteraction {
    ChangeState {
        instance_daemon: InstanceDaemon,
        project_name: String,
        instance_name: String,
        run_change: RunChange,
    },
    Expansion(WidgetGroupExpansionMessage),
}

impl IncusInstancesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<IncusInstancesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        Subscription::batch(WATCHED_DAEMONS.into_iter().flat_map(|instance_daemon| {
            [
                snapshot_subscription::<InstanceDaemonProbe>(instance_daemon)
                    .with(instance_daemon)
                    .map(|(instance_daemon, delivery)| {
                        IncusInstancesMessage::Probe(instance_daemon, delivery)
                    }),
                demand.while_face_shown(
                    snapshot_subscription::<FamilyListings<DaemonInstanceListing>>(instance_daemon)
                        .with(instance_daemon)
                        .map(|(instance_daemon, delivery)| {
                            IncusInstancesMessage::Listings(instance_daemon, delivery)
                        }),
                ),
                demand.while_rows_shown(
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
            IncusInstancesMessage::Interaction(IncusInstancesInteraction::ChangeState {
                instance_daemon,
                project_name,
                instance_name,
                run_change,
            }) => Task::future(async move {
                change_instance_state(instance_daemon, &project_name, &instance_name, run_change)
                    .await
                    .err()
            })
            .and_then(|refusal| Task::done(IncusInstancesMessage::StateChangeRefused(refusal))),
            IncusInstancesMessage::Interaction(IncusInstancesInteraction::Expansion(
                expansion_message,
            )) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn probe_availability(&self, instance_daemon: InstanceDaemon) -> Option<Availability> {
        self.probes
            .get(&instance_daemon)?
            .get()
            .map(|InstanceDaemonProbe(availability)| *availability)
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &IncusInstanceTarget) -> Vec<Section> {
        let instance_daemon = target.instance_daemon;
        if let Some(detail_reading) = self.details.get(&instance_daemon) {
            if let Some(instances) = detail_reading.get() {
                return instances
                    .details
                    .iter()
                    .find(|instance| instance.listing.instance_name == target.instance_name)
                    .map(|instance| {
                        detail_reading.hover_card_with(
                            IncusInstanceRow {
                                instance_daemon,
                                instance,
                            }
                            .hover_card(),
                        )
                    })
                    .unwrap_or_default();
            }
            if let Some(read_failure) = detail_reading.fetch_failure() {
                return UnreadableDaemonRow {
                    instance_daemon,
                    read_failure,
                }
                .hover_card();
            }
            return Vec::new();
        }
        let Some(listing_reading) = self.listings.get(&instance_daemon) else {
            return Vec::new();
        };
        if let Some(read_failure) = listing_reading.fetch_failure() {
            return UnreadableDaemonRow {
                instance_daemon,
                read_failure,
            }
            .hover_card();
        }
        let Some(listing) = listing_reading.get().and_then(|listings| {
            listings
                .listings
                .iter()
                .find(|listing| listing.instance_name == target.instance_name)
        }) else {
            return Vec::new();
        };
        listing_reading.hover_card_with(
            IncusInstanceListingRow {
                instance_daemon,
                listing,
            }
            .hover_card(),
        )
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, IncusInstancesInteraction> {
        let rows = WATCHED_DAEMONS
            .into_iter()
            .flat_map(|instance_daemon| self.daemon_rows(instance_daemon));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::IncusInstances,
            &self.expansion,
            IncusInstancesInteraction::Expansion,
            rows,
        )
        .face(WidgetGroupFace::Glyph {
            glyph: Phosphor::Package(Weight::Regular).into(),
            state: if WATCHED_DAEMONS.into_iter().any(|instance_daemon| {
                self.probe_availability(instance_daemon) == Some(Availability::Available)
            }) {
                ReadingDisplayState::On
            } else {
                ReadingDisplayState::Off
            },
        })
    }

    fn daemon_rows(
        &self,
        instance_daemon: InstanceDaemon,
    ) -> Vec<GroupItem<'_, IncusInstancesInteraction>> {
        if let Some(instances) = self
            .details
            .get(&instance_daemon)
            .and_then(SubscribedSnapshot::get)
        {
            return instances
                .details
                .iter()
                .map(|instance| {
                    let row = GroupItem::reading(
                        IncusInstanceRow {
                            instance_daemon,
                            instance,
                        }
                        .reading_row(),
                        IncusInstanceTarget {
                            instance_daemon,
                            instance_name: Cow::Owned(instance.listing.instance_name.clone()),
                        },
                    );
                    instance_power_switch(row, instance_daemon, &instance.listing)
                })
                .collect();
        }
        let Some(listing_reading) = self.listings.get(&instance_daemon) else {
            return Vec::new();
        };
        if let Some(read_failure) = listing_reading.fetch_failure() {
            return vec![GroupItem::reading(
                UnreadableDaemonRow {
                    instance_daemon,
                    read_failure,
                }
                .reading_row(),
                IncusInstanceTarget {
                    instance_daemon,
                    instance_name: Cow::Borrowed(instance_daemon.product_name()),
                },
            )];
        }
        listing_reading
            .get()
            .map(|listings| listings.listings.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|listing| {
                let row = GroupItem::reading(
                    IncusInstanceListingRow {
                        instance_daemon,
                        listing,
                    }
                    .reading_row(),
                    IncusInstanceTarget {
                        instance_daemon,
                        instance_name: Cow::Owned(listing.instance_name.clone()),
                    },
                );
                instance_power_switch(row, instance_daemon, listing)
            })
            .collect()
    }
}
