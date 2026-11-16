use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelOccurrence,
    journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrors,
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    widget_groups::{
        group_summary::GroupSummary,
        journal_events_group::{
            JournalEventsInteraction,
            coredump_log::{
                COREDUMP_LOG_GLYPH, coredump_log_indicator, coredump_log_panel_visibility,
                coredump_log_state,
            },
            kernel_error_log::{
                KERNEL_LOG_GLYPH, kernel_log_indicator, kernel_log_panel_visibility,
                kernel_log_state,
            },
            oom_kill_log::{
                OOM_KILL_LOG_GLYPH, oom_kill_log_indicator, oom_kill_log_panel_visibility,
                oom_kill_log_state,
            },
            service_error_log::{
                SERVICE_ERRORS_GLYPH, service_errors_indicator, service_errors_panel_visibility,
                service_errors_state,
            },
        },
        terminal_command::TerminalCommand,
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

pub struct JournalScopeGroup {
    pub(super) unit_scope: SystemdUnitScope,
    pub(super) kernel: SubscribedSnapshot<Vec<KernelOccurrence>>,
    pub(super) service_errors: SubscribedSnapshot<ServiceErrors>,
    pub(super) coredumps: SubscribedSnapshot<Vec<CoredumpEntry>>,
    pub(super) oom_kills: SubscribedSnapshot<Vec<OomKillEntry>>,
    pub(super) expansion: WidgetGroupExpansion,
}

impl JournalScopeGroup {
    const fn family(&self) -> WidgetGroupFamily {
        match self.unit_scope {
            SystemdUnitScope::SystemWide => WidgetGroupFamily::SystemJournal,
            SystemdUnitScope::UserSession => WidgetGroupFamily::UserJournal,
        }
    }

    pub(super) fn group_content<'a>(
        &'a self,
        config: &'a Config,
        clock: Clock,
    ) -> WidgetGroupContent<'a> {
        let unit_scope = self.unit_scope;
        let mut rows = Vec::new();
        self.push_kernel_log_indicators(&mut rows, clock, config);
        self.push_service_errors_indicators(&mut rows, config);
        self.push_coredump_log_indicators(&mut rows, clock, config);
        self.push_oom_kill_log_indicators(&mut rows, clock, config);
        WidgetGroupContent::expandable(
            self.family(),
            GroupSummary::glyph(self.icon(), self.state(clock)),
            &self.expansion,
            move |expansion_message| {
                JournalEventsInteraction::Expansion(unit_scope, expansion_message)
            },
            rows,
        )
    }

    fn push_kernel_log_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        config: &'a Config,
    ) {
        match &self.kernel {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let kernel_indicator = kernel_log_indicator(held_snapshot);
                if kernel_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_kernel_log_indicator(config, kernel_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let kernel_indicator = troubled_snapshot.shown_over(
                    kernel_log_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if kernel_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_kernel_log_indicator(config, kernel_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_kernel_log_indicator(
                    config,
                    placeholder.indicator(KERNEL_LOG_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_service_errors_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        config: &'a Config,
    ) {
        match &self.service_errors {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let service_errors_row = service_errors_indicator(held_snapshot);
                if service_errors_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(
                        self.wired_service_errors_indicator(service_errors_row)
                            .on_press(
                                TerminalCommand::JOURNAL_PRESS_HINT,
                                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                                    config,
                                    held_snapshot.follow_log_args(),
                                )),
                            ),
                    );
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let held_snapshot = &troubled_snapshot.held_snapshot;
                let service_errors_row = service_errors_indicator(held_snapshot);
                if service_errors_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(
                        self.wired_service_errors_indicator(service_errors_row)
                            .on_press(
                                TerminalCommand::JOURNAL_PRESS_HINT,
                                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                                    config,
                                    held_snapshot.follow_log_args(),
                                )),
                            ),
                    );
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_service_errors_indicator(
                    placeholder.indicator(SERVICE_ERRORS_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_coredump_log_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        config: &'a Config,
    ) {
        match &self.coredumps {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let coredump_indicator = coredump_log_indicator(held_snapshot);
                if coredump_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_coredump_log_indicator(config, coredump_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let coredump_indicator = troubled_snapshot.shown_over(
                    coredump_log_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if coredump_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_coredump_log_indicator(config, coredump_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_coredump_log_indicator(
                    config,
                    placeholder.indicator(COREDUMP_LOG_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_oom_kill_log_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        config: &'a Config,
    ) {
        match &self.oom_kills {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let oom_kill_indicator = oom_kill_log_indicator(held_snapshot);
                if oom_kill_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_oom_kill_log_indicator(config, oom_kill_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let oom_kill_indicator = troubled_snapshot.shown_over(
                    oom_kill_log_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if oom_kill_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.wired_oom_kill_log_indicator(config, oom_kill_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.wired_oom_kill_log_indicator(
                    config,
                    placeholder.indicator(OOM_KILL_LOG_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn journal_kind_states(&self, clock: Clock) -> [Option<IndicatorState>; 4] {
        [
            self.kernel.shown_state(
                |snapshot| kernel_log_state(snapshot),
                |snapshot| kernel_log_panel_visibility(snapshot),
                clock,
            ),
            self.service_errors.shown_state(
                service_errors_state,
                service_errors_panel_visibility,
                clock,
            ),
            self.coredumps.shown_state(
                |snapshot| coredump_log_state(snapshot),
                |snapshot| coredump_log_panel_visibility(snapshot),
                clock,
            ),
            self.oom_kills.shown_state(
                |snapshot| oom_kill_log_state(snapshot),
                |snapshot| oom_kill_log_panel_visibility(snapshot),
                clock,
            ),
        ]
    }
}

impl JournalScopeGroup {
    fn icon(&self) -> IconGlyph {
        match self.unit_scope {
            SystemdUnitScope::SystemWide => Phosphor::Scroll(Weight::Regular).into(),
            SystemdUnitScope::UserSession => Phosphor::UserList(Weight::Regular).into(),
        }
    }

    fn state(&self, clock: Clock) -> IndicatorState {
        self.journal_kind_states(clock)
            .into_iter()
            .flatten()
            .map(IndicatorState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
