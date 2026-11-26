use iced::{Subscription, Task};
pub use journal_scope_group::JournalScopeGroup;

mod coredump_indicator;
mod journal_scope_group;
mod kernel_error_indicator;
mod oom_kill_indicator;
mod service_error_indicator;

use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    systemd_units_journal::{
        SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelEntry,
        journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrorEntry,
    },
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_indicators::{
        group_indicator::PanelVisibility,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        terminal_command::TerminalCommand,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub const MESSAGE_CAP: usize = 100;

pub(super) fn shown_while_alerting(
    entry_severities: impl IntoIterator<Item = Severity>,
) -> PanelVisibility {
    if entry_severities
        .into_iter()
        .any(|entry_severity| entry_severity >= Severity::Warning)
    {
        PanelVisibility::Shown
    } else {
        PanelVisibility::Hidden
    }
}

pub struct JournalEventsGroup {
    system_journal: JournalScopeGroup,
    user_journal: JournalScopeGroup,
}

impl Default for JournalEventsGroup {
    fn default() -> Self {
        Self {
            system_journal: JournalScopeGroup {
                unit_scope: SystemdUnitScope::SystemWide,
                kernel: SubscribedSnapshot::default(),
                service_errors: SubscribedSnapshot::default(),
                coredumps: SubscribedSnapshot::default(),
                oom_kills: SubscribedSnapshot::default(),
                expansion: IndicatorGroupExpansion::default(),
            },
            user_journal: JournalScopeGroup {
                unit_scope: SystemdUnitScope::UserSession,
                kernel: SubscribedSnapshot::default(),
                service_errors: SubscribedSnapshot::default(),
                coredumps: SubscribedSnapshot::default(),
                oom_kills: SubscribedSnapshot::default(),
                expansion: IndicatorGroupExpansion::default(),
            },
        }
    }
}

#[derive(Debug)]
pub enum JournalEventsMessage {
    Kernel(SnapshotDelivery<Vec<KernelEntry>>),
    ServiceErrors(SnapshotDelivery<Vec<ServiceErrorEntry>>),
    Coredumps(SnapshotDelivery<Vec<CoredumpEntry>>),
    OomKills(SnapshotDelivery<Vec<OomKillEntry>>),
}

#[derive(Debug, Clone)]
pub enum JournalEventsInteraction {
    OpenLog(TerminalCommand),
    Expansion(SystemdUnitScope, IndicatorGroupExpansionMessage),
}

fn scoped_delivery<Snapshot>(
    delivery: &SnapshotDelivery<Snapshot>,
    unit_scope: SystemdUnitScope,
    part_in_scope: impl Fn(&Snapshot, SystemdUnitScope) -> Snapshot,
) -> SnapshotDelivery<Snapshot>
where
    Snapshot: SnapshotSource,
    Snapshot::Error: Clone,
{
    match delivery {
        SnapshotDelivery::SourceUnavailable => SnapshotDelivery::SourceUnavailable,
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence,
        } => SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch: snapshot_fetch
                .as_ref()
                .map(|snapshot| part_in_scope(snapshot, unit_scope))
                .map_err(Clone::clone),
            delivery_cadence: *delivery_cadence,
        },
        SnapshotDelivery::SnapshotUnchanged => SnapshotDelivery::SnapshotUnchanged,
    }
}

fn coredumps_in_scope(
    coredumps: &[CoredumpEntry],
    unit_scope: SystemdUnitScope,
) -> Vec<CoredumpEntry> {
    coredumps
        .iter()
        .filter(|coredump| coredump.unit.unit_scope() == unit_scope)
        .cloned()
        .collect()
}

fn service_errors_in_scope(
    service_errors: &[ServiceErrorEntry],
    unit_scope: SystemdUnitScope,
) -> Vec<ServiceErrorEntry> {
    service_errors
        .iter()
        .filter(|service_error| service_error.unit.unit_scope() == unit_scope)
        .cloned()
        .collect()
}

fn oom_kills_in_scope(
    oom_kills: &[OomKillEntry],
    unit_scope: SystemdUnitScope,
) -> Vec<OomKillEntry> {
    oom_kills
        .iter()
        .filter(|oom_kill| oom_kill.unit.unit_scope() == unit_scope)
        .cloned()
        .collect()
}

impl JournalEventsGroup {
    pub fn collapse(&mut self) {
        self.system_journal.expansion.collapse();
        self.user_journal.expansion.collapse();
    }

    pub fn subscription() -> Subscription<JournalEventsMessage> {
        Subscription::batch([
            snapshot_subscription::<Vec<KernelEntry>>(()).map(JournalEventsMessage::Kernel),
            snapshot_subscription::<Vec<ServiceErrorEntry>>(())
                .map(JournalEventsMessage::ServiceErrors),
            snapshot_subscription::<Vec<CoredumpEntry>>(()).map(JournalEventsMessage::Coredumps),
            snapshot_subscription::<Vec<OomKillEntry>>(()).map(JournalEventsMessage::OomKills),
        ])
    }

    fn scope_groups_mut(&mut self) -> [&mut JournalScopeGroup; 2] {
        [&mut self.system_journal, &mut self.user_journal]
    }

    pub fn update(
        &mut self,
        journal_message: JournalEventsMessage,
        clock: Clock,
    ) -> Task<JournalEventsMessage> {
        match journal_message {
            JournalEventsMessage::Kernel(delivery) => {
                self.system_journal.kernel.receive(delivery, clock);
            }
            JournalEventsMessage::ServiceErrors(delivery) => {
                for scope_group in self.scope_groups_mut() {
                    scope_group.service_errors.receive(
                        scoped_delivery(
                            &delivery,
                            scope_group.unit_scope,
                            |service_errors, unit_scope| {
                                service_errors_in_scope(service_errors, unit_scope)
                            },
                        ),
                        clock,
                    );
                }
            }
            JournalEventsMessage::Coredumps(delivery) => {
                for scope_group in self.scope_groups_mut() {
                    scope_group.coredumps.receive(
                        scoped_delivery(
                            &delivery,
                            scope_group.unit_scope,
                            |coredumps, unit_scope| coredumps_in_scope(coredumps, unit_scope),
                        ),
                        clock,
                    );
                }
            }
            JournalEventsMessage::OomKills(delivery) => {
                for scope_group in self.scope_groups_mut() {
                    scope_group.oom_kills.receive(
                        scoped_delivery(
                            &delivery,
                            scope_group.unit_scope,
                            |oom_kills, unit_scope| oom_kills_in_scope(oom_kills, unit_scope),
                        ),
                        clock,
                    );
                }
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: JournalEventsInteraction,
    ) -> Task<JournalEventsMessage> {
        match interaction {
            JournalEventsInteraction::OpenLog(journal) => return journal.open(),
            JournalEventsInteraction::Expansion(
                SystemdUnitScope::SystemWide,
                expansion_message,
            ) => {
                self.system_journal.expansion.update(expansion_message);
            }
            JournalEventsInteraction::Expansion(
                SystemdUnitScope::UserSession,
                expansion_message,
            ) => {
                self.user_journal.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn system_journal_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        self.system_journal.group_content(config, clock.wall)
    }

    #[must_use]
    pub fn user_journal_content<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> IndicatorGroupContent<'view> {
        self.user_journal.group_content(config, clock.wall)
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::SystemTime};

    use sottomano_desktop_sources::systemd_units_journal::{
        journal_coredumps::{CoreDumpingSignal, CoreFileStorage, CoredumpOwner},
        journal_record_fields::OwningUnit,
    };

    use super::*;

    fn coredump_owned_by(unit: CoredumpOwner) -> CoredumpEntry {
        CoredumpEntry {
            at: SystemTime::UNIX_EPOCH,
            command: "crashing".to_owned(),
            executable: PathBuf::from("/usr/bin/crashing"),
            signal: CoreDumpingSignal::Abort,
            pid: 42,
            unit,
            core_file: CoreFileStorage::NotOnDisk,
            severity: Severity::Critical,
        }
    }

    #[test]
    fn unitless_coredumps_land_in_the_system_journal() {
        let coredumps = [
            coredump_owned_by(CoredumpOwner::Unit(OwningUnit::System(
                "sshd.service".to_owned(),
            ))),
            coredump_owned_by(CoredumpOwner::Unit(OwningUnit::User(
                "app-firefox.scope".to_owned(),
            ))),
            coredump_owned_by(CoredumpOwner::OutsideUnits),
        ];
        let unit_names_in = |unit_scope| {
            coredumps_in_scope(&coredumps, unit_scope)
                .into_iter()
                .map(|coredump| coredump.unit)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            unit_names_in(SystemdUnitScope::SystemWide),
            [
                CoredumpOwner::Unit(OwningUnit::System("sshd.service".to_owned())),
                CoredumpOwner::OutsideUnits
            ]
        );
        assert_eq!(
            unit_names_in(SystemdUnitScope::UserSession),
            [CoredumpOwner::Unit(OwningUnit::User(
                "app-firefox.scope".to_owned()
            ))]
        );
    }
}
