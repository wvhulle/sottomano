use std::{borrow::Cow, collections::BTreeMap};

use chrono::Locale;
use iced::{Subscription, Task};
use sottomano_desktop_sinks::incus_instances::{InstanceStateChange, change_instance_state};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    backing_service_availability::Availability,
    incus_instances::{
        DaemonInstance, DaemonInstanceListing, DaemonInstanceListings, DaemonInstancesReadout,
        InstanceDaemon, InstanceKind,
    },
    podman_container_engine::libpod_running_containers::EngineReadError,
};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        process_usage_cells::approximate_memory_text,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const WATCHED_DAEMONS: [InstanceDaemon; 2] = [InstanceDaemon::Incus, InstanceDaemon::Lxd];
const START_HINT: &str = "start the instance";
const STOP_HINT: &str = "stop the instance";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncusInstanceTarget {
    instance_daemon: InstanceDaemon,
    instance_name: Cow<'static, str>,
}

impl IncusInstanceTarget {
    #[must_use]
    pub const fn of_instance(
        instance_daemon: InstanceDaemon,
        instance_name: Cow<'static, str>,
    ) -> Self {
        Self {
            instance_daemon,
            instance_name,
        }
    }
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

fn instance_start_press<'a>(
    row: GroupItem<'a, IncusInstancesMessage>,
    instance_daemon: InstanceDaemon,
    project_name: &str,
    instance_name: &str,
    running: bool,
) -> GroupItem<'a, IncusInstancesMessage> {
    let state_change = if running {
        InstanceStateChange::Stop
    } else {
        InstanceStateChange::Start
    };
    if running {
        row.on_right_press(
            STOP_HINT,
            IncusInstancesMessage::ChangeState {
                instance_daemon,
                project_name: project_name.to_owned(),
                instance_name: instance_name.to_owned(),
                state_change,
            },
        )
    } else {
        row.on_press(
            START_HINT,
            IncusInstancesMessage::ChangeState {
                instance_daemon,
                project_name: project_name.to_owned(),
                instance_name: instance_name.to_owned(),
                state_change,
            },
        )
    }
}

struct IncusInstanceRow<'a> {
    instance_daemon: InstanceDaemon,
    instance: &'a DaemonInstance,
}

impl GroupItemReading for IncusInstanceRow<'_> {
    fn name(&self) -> &'static str {
        "Instance"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.instance.instance_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        instance_icon(self.instance.kind)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        if !self.instance.running {
            return Some(self.instance.status_text.to_lowercase());
        }
        self.instance
            .global_addresses
            .first()
            .cloned()
            .or_else(|| self.instance.memory_in_use.map(approximate_memory_text))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.instance.running {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let instance = self.instance;
        let mut detail_entries = vec![
            LabeledValue::new("Kind", kind_text(instance.kind)),
            LabeledValue::new("Status", instance.status_text.clone()),
            LabeledValue::new("Daemon", self.instance_daemon.product_name()),
            LabeledValue::new("Project", instance.project_name.clone()),
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
        HoverCardBody::dictionary(detail_entries)
    }
}

struct IncusInstanceListingRow<'a> {
    instance_daemon: InstanceDaemon,
    listing: &'a DaemonInstanceListing,
}

impl GroupItemReading for IncusInstanceListingRow<'_> {
    fn name(&self) -> &'static str {
        "Instance"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.listing.instance_name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        instance_icon(self.listing.kind)
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(self.listing.status_text.to_lowercase())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.listing.running {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Kind", kind_text(self.listing.kind)),
            LabeledValue::new("Status", self.listing.status_text.clone()),
            LabeledValue::new("Daemon", self.instance_daemon.product_name()),
            LabeledValue::new("Project", self.listing.project_name.clone()),
        ])
    }
}

struct UnreadableDaemonRow {
    instance_daemon: InstanceDaemon,
    failure: EngineReadError,
}

impl GroupItemReading for UnreadableDaemonRow {
    fn name(&self) -> &'static str {
        "Instance daemon"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.instance_daemon.product_name().into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(match self.failure {
            EngineReadError::SocketDenied => "no access: socket denied".to_owned(),
            _ => "unreadable".to_owned(),
        })
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Problem", self.failure.to_string()),
            LabeledValue::new(
                "Socket",
                self.instance_daemon.socket_path().display().to_string(),
            ),
        ])
    }
}

#[derive(Default)]
pub struct IncusInstancesGroup {
    probes: BTreeMap<InstanceDaemon, LatestSnapshot<Availability, HidesFetchFailures>>,
    listings: BTreeMap<InstanceDaemon, LatestSnapshot<DaemonInstanceListings, HidesFetchFailures>>,
    details: BTreeMap<InstanceDaemon, LatestSnapshot<DaemonInstancesReadout, HidesFetchFailures>>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum IncusInstancesMessage {
    Probe(InstanceDaemon, SnapshotDelivery<Availability>),
    Listings(InstanceDaemon, SnapshotDelivery<DaemonInstanceListings>),
    Instances(InstanceDaemon, SnapshotDelivery<DaemonInstancesReadout>),
    ChangeState {
        instance_daemon: InstanceDaemon,
        project_name: String,
        instance_name: String,
        state_change: InstanceStateChange,
    },
    StateChangeRefused(String),
    Expansion(WidgetGroupExpansionMessage),
}

impl IncusInstancesGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, demand: GroupSourceDemand) -> Subscription<IncusInstancesMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        let mut subscriptions: Vec<Subscription<IncusInstancesMessage>> = Vec::new();
        for instance_daemon in WATCHED_DAEMONS {
            subscriptions.extend([
                snapshot_subscription::<Availability>(instance_daemon)
                    .with(instance_daemon)
                    .map(|(instance_daemon, delivery)| {
                        IncusInstancesMessage::Probe(instance_daemon, delivery)
                    }),
                demand.while_face_shown(
                    snapshot_subscription::<DaemonInstanceListings>(instance_daemon)
                        .with(instance_daemon)
                        .map(|(instance_daemon, delivery)| {
                            IncusInstancesMessage::Listings(instance_daemon, delivery)
                        }),
                ),
                demand.while_rows_shown(
                    snapshot_subscription::<DaemonInstancesReadout>(instance_daemon)
                        .with(instance_daemon)
                        .map(|(instance_daemon, delivery)| {
                            IncusInstancesMessage::Instances(instance_daemon, delivery)
                        }),
                ),
            ]);
        }
        Subscription::batch(subscriptions)
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
            IncusInstancesMessage::ChangeState {
                instance_daemon,
                project_name,
                instance_name,
                state_change,
            } => Task::future(async move {
                change_instance_state(instance_daemon, &project_name, &instance_name, state_change)
                    .await
                    .err()
            })
            .and_then(|refusal| Task::done(IncusInstancesMessage::StateChangeRefused(refusal))),
            IncusInstancesMessage::StateChangeRefused(refusal) => {
                tracing::warn!("incus instance state: {refusal}");
                Task::none()
            }
            IncusInstancesMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
        }
    }

    fn probe_availability(&self, instance_daemon: InstanceDaemon) -> Option<Availability> {
        self.probes.get(&instance_daemon)?.get().copied()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &IncusInstanceTarget, clock: Clock) -> Option<HoverCard> {
        if let Some(detail_reading) = self.details.get(&target.instance_daemon) {
            match detail_reading.get()? {
                DaemonInstancesReadout::Instances(instances) => {
                    let instance = instances
                        .iter()
                        .find(|instance| instance.instance_name == target.instance_name)?;
                    return detail_reading.hover_card_with(
                        IncusInstanceRow {
                            instance_daemon: target.instance_daemon,
                            instance,
                        }
                        .hover_card(clock),
                    );
                }
                DaemonInstancesReadout::Unreadable(failure) => {
                    return detail_reading.hover_card_with(
                        UnreadableDaemonRow {
                            instance_daemon: target.instance_daemon,
                            failure: failure.clone(),
                        }
                        .hover_card(clock),
                    );
                }
            }
        }
        let listing_reading = self.listings.get(&target.instance_daemon)?;
        match listing_reading.get()? {
            DaemonInstanceListings::Instances(listings) => {
                let listing = listings
                    .iter()
                    .find(|listing| listing.instance_name == target.instance_name)?;
                listing_reading.hover_card_with(
                    IncusInstanceListingRow {
                        instance_daemon: target.instance_daemon,
                        listing,
                    }
                    .hover_card(clock),
                )
            }
            DaemonInstanceListings::Unreadable(failure) => listing_reading.hover_card_with(
                UnreadableDaemonRow {
                    instance_daemon: target.instance_daemon,
                    failure: failure.clone(),
                }
                .hover_card(clock),
            ),
        }
    }

    fn probe_delivered_for_any_daemon(&self) -> bool {
        WATCHED_DAEMONS
            .into_iter()
            .any(|instance_daemon| self.probe_availability(instance_daemon).is_some())
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, IncusInstancesMessage> {
        let rows = WATCHED_DAEMONS
            .into_iter()
            .flat_map(|instance_daemon| self.daemon_rows(instance_daemon));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::IncusInstances,
            &self.expansion,
            IncusInstancesMessage::Expansion,
            rows,
        )
        .face(WidgetGroupFace::IconColoredBy {
            glyph: Phosphor::Package(Weight::Regular).into(),
            reading: self,
        })
    }

    fn daemon_rows(
        &self,
        instance_daemon: InstanceDaemon,
    ) -> Vec<GroupItem<'_, IncusInstancesMessage>> {
        if let Some(DaemonInstancesReadout::Instances(instances)) = self
            .details
            .get(&instance_daemon)
            .and_then(|reading| reading.get())
        {
            return instances
                .iter()
                .map(|instance| {
                    let row = GroupItem::reading(
                        IncusInstanceRow {
                            instance_daemon,
                            instance,
                        },
                        IncusInstanceTarget::of_instance(
                            instance_daemon,
                            Cow::Owned(instance.instance_name.clone()),
                        ),
                    );
                    instance_start_press(
                        row,
                        instance_daemon,
                        &instance.project_name,
                        &instance.instance_name,
                        instance.running,
                    )
                })
                .collect();
        }
        let Some(listings) = self
            .listings
            .get(&instance_daemon)
            .and_then(|reading| reading.get())
        else {
            return Vec::new();
        };
        match listings {
            DaemonInstanceListings::Instances(listings) => listings
                .iter()
                .map(|listing| {
                    let row = GroupItem::reading(
                        IncusInstanceListingRow {
                            instance_daemon,
                            listing,
                        },
                        IncusInstanceTarget::of_instance(
                            instance_daemon,
                            Cow::Owned(listing.instance_name.clone()),
                        ),
                    );
                    instance_start_press(
                        row,
                        instance_daemon,
                        &listing.project_name,
                        &listing.instance_name,
                        listing.running,
                    )
                })
                .collect(),
            DaemonInstanceListings::Unreadable(EngineReadError::SocketMissing) => Vec::new(),
            DaemonInstanceListings::Unreadable(failure) => vec![GroupItem::reading(
                UnreadableDaemonRow {
                    instance_daemon,
                    failure: failure.clone(),
                },
                IncusInstanceTarget::of_instance(
                    instance_daemon,
                    Cow::Borrowed(instance_daemon.product_name()),
                ),
            )],
        }
    }
}

impl GroupItemReading for IncusInstancesGroup {
    fn name(&self) -> &'static str {
        "Incus instances"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        "Incus instances".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Package(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        let available_daemons: Vec<&'static str> = WATCHED_DAEMONS
            .into_iter()
            .filter(|instance_daemon| {
                self.probe_availability(*instance_daemon) == Some(Availability::Available)
            })
            .map(InstanceDaemon::product_name)
            .collect();
        if available_daemons.is_empty() {
            return self
                .probe_delivered_for_any_daemon()
                .then_some("no daemon socket".to_owned());
        }
        Some(available_daemons.join(", "))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        let any_daemon_available = WATCHED_DAEMONS.into_iter().any(|instance_daemon| {
            self.probe_availability(instance_daemon) == Some(Availability::Available)
        });
        if any_daemon_available {
            ReadingDisplayState::On
        } else {
            ReadingDisplayState::Off
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(
            WATCHED_DAEMONS
                .into_iter()
                .filter_map(|instance_daemon| {
                    let availability = self.probe_availability(instance_daemon)?;
                    Some(LabeledValue::new(
                        instance_daemon.product_name(),
                        match availability {
                            Availability::Available => "socket present",
                            Availability::Unavailable => "socket missing",
                        },
                    ))
                })
                .collect::<Vec<LabeledValue>>(),
        )
    }
}
