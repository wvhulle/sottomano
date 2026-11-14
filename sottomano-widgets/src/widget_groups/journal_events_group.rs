use iced::{Subscription, Task};
pub use journal_scope_group::JournalScopeGroup;

mod coredump_log;
mod journal_scope_group;
mod kernel_error_log;
mod oom_kill_log;
mod service_error_log;

use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    systemd_units_journal::{
        SystemdUnitScope, journal_coredumps::CoredumpEntry,
        journal_kernel_errors::KernelOccurrence, journal_oom_kills::OomKillEntry,
        journal_record_fields::OwningUnit, journal_service_errors::ServiceErrors,
    },
};
use sottomano_toml_config::{Config, severity::Severity};

use crate::{
    background_work::snapshot_subscription,
    clock::Clock,
    group_items::group_item_reading::PanelVisibility,
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        terminal_command::TerminalCommand,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
    widgets::latest_snapshot::LatestSnapshot,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalTarget {
    Kernel,
    ServiceErrors(SystemdUnitScope),
    Coredumps(SystemdUnitScope),
    OomKills(SystemdUnitScope),
}

impl From<JournalTarget> for HoverTarget {
    fn from(target: JournalTarget) -> Self {
        HoverTarget::Journal(target)
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
                kernel: LatestSnapshot::default(),
                service_errors: LatestSnapshot::default(),
                coredumps: LatestSnapshot::default(),
                oom_kills: LatestSnapshot::default(),
                expansion: WidgetGroupExpansion::default(),
            },
            user_journal: JournalScopeGroup {
                unit_scope: SystemdUnitScope::UserSession,
                kernel: LatestSnapshot::SourceUnavailable,
                service_errors: LatestSnapshot::default(),
                coredumps: LatestSnapshot::default(),
                oom_kills: LatestSnapshot::default(),
                expansion: WidgetGroupExpansion::default(),
            },
        }
    }
}

#[derive(Debug)]
pub enum JournalEventsMessage {
    Kernel(SnapshotDelivery<Vec<KernelOccurrence>>),
    ServiceErrors(SnapshotDelivery<ServiceErrors>),
    Coredumps(SnapshotDelivery<Vec<CoredumpEntry>>),
    OomKills(SnapshotDelivery<Vec<OomKillEntry>>),
    Interaction(JournalEventsInteraction),
}

#[derive(Debug, Clone)]
pub enum JournalEventsInteraction {
    OpenLog(TerminalCommand),
    Expansion(SystemdUnitScope, WidgetGroupExpansionMessage),
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
        SnapshotDelivery::InternetUnreachable(unreachable) => {
            SnapshotDelivery::InternetUnreachable(unreachable.clone())
        }
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
        .filter(|coredump| {
            coredump
                .unit
                .as_ref()
                .map_or(SystemdUnitScope::SystemWide, OwningUnit::unit_scope)
                == unit_scope
        })
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
            snapshot_subscription::<Vec<KernelOccurrence>>(()).map(JournalEventsMessage::Kernel),
            snapshot_subscription::<ServiceErrors>(()).map(JournalEventsMessage::ServiceErrors),
            snapshot_subscription::<Vec<CoredumpEntry>>(()).map(JournalEventsMessage::Coredumps),
            snapshot_subscription::<Vec<OomKillEntry>>(()).map(JournalEventsMessage::OomKills),
        ])
    }

    fn scope_groups_mut(&mut self) -> [&mut JournalScopeGroup; 2] {
        [&mut self.system_journal, &mut self.user_journal]
    }

    const fn scope_group(&self, unit_scope: SystemdUnitScope) -> &JournalScopeGroup {
        match unit_scope {
            SystemdUnitScope::SystemWide => &self.system_journal,
            SystemdUnitScope::UserSession => &self.user_journal,
        }
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
                        scoped_delivery(&delivery, scope_group.unit_scope, ServiceErrors::owned_by),
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
            JournalEventsMessage::Interaction(JournalEventsInteraction::OpenLog(journal)) => {
                return journal.open();
            }
            JournalEventsMessage::Interaction(JournalEventsInteraction::Expansion(
                SystemdUnitScope::SystemWide,
                expansion_message,
            )) => {
                self.system_journal.expansion.update(expansion_message);
            }
            JournalEventsMessage::Interaction(JournalEventsInteraction::Expansion(
                SystemdUnitScope::UserSession,
                expansion_message,
            )) => {
                self.user_journal.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: JournalTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            JournalTarget::Kernel => self.system_journal.kernel.hover_card(clock),
            JournalTarget::ServiceErrors(unit_scope) => self
                .scope_group(unit_scope)
                .service_errors
                .hover_card(clock),
            JournalTarget::Coredumps(unit_scope) => {
                self.scope_group(unit_scope).coredumps.hover_card(clock)
            }
            JournalTarget::OomKills(unit_scope) => {
                self.scope_group(unit_scope).oom_kills.hover_card(clock)
            }
        }
    }

    #[must_use]
    pub fn system_journal_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, JournalEventsInteraction> {
        self.system_journal.group_content(config)
    }

    #[must_use]
    pub fn user_journal_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, JournalEventsInteraction> {
        self.user_journal.group_content(config)
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::SystemTime};

    use sottomano_desktop_sources::systemd_units_journal::journal_coredumps::CoreDumpingSignal;

    use super::*;

    fn coredump_owned_by(unit: Option<OwningUnit>) -> CoredumpEntry {
        CoredumpEntry {
            at: SystemTime::UNIX_EPOCH,
            command: "crashing".to_owned(),
            executable: PathBuf::from("/usr/bin/crashing"),
            signal: CoreDumpingSignal::Abort,
            pid: 42,
            unit,
            stored_file: None,
            severity: Severity::Critical,
        }
    }

    #[test]
    fn unitless_coredumps_land_in_the_system_journal() {
        let coredumps = [
            coredump_owned_by(Some(OwningUnit::System("sshd.service".to_owned()))),
            coredump_owned_by(Some(OwningUnit::User("app-firefox.scope".to_owned()))),
            coredump_owned_by(None),
        ];
        let unit_names_in = |unit_scope| {
            coredumps_in_scope(&coredumps, unit_scope)
                .into_iter()
                .map(|coredump| coredump.unit.map(|unit| unit.name().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            unit_names_in(SystemdUnitScope::SystemWide),
            [Some("sshd.service".to_owned()), None]
        );
        assert_eq!(
            unit_names_in(SystemdUnitScope::UserSession),
            [Some("app-firefox.scope".to_owned())]
        );
    }
}
