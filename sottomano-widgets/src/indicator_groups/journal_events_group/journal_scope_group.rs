use std::time::SystemTime;

use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelEntry,
    journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrorEntry,
};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        journal_events_group::{
            JournalEventsInteraction,
            coredump_log::{
                COREDUMP_LOG_GLYPH, coredump_log_indicator, coredump_log_panel_visibility,
                coredump_log_state, live_coredumps,
            },
            kernel_error_log::{
                KERNEL_LOG_GLYPH, kernel_log_indicator, kernel_log_panel_visibility,
                kernel_log_state, live_kernel_occurrences,
            },
            oom_kill_log::{
                OOM_KILL_LOG_GLYPH, live_oom_kills, oom_kill_log_indicator,
                oom_kill_log_panel_visibility, oom_kill_log_state,
            },
            service_error_log::{
                SERVICE_ERRORS_GLYPH, live_service_errors, service_errors_indicator,
                service_errors_panel_visibility, service_errors_state,
            },
        },
        terminal_command::TerminalCommand,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub struct JournalScopeGroup {
    pub(super) unit_scope: SystemdUnitScope,
    pub(super) kernel: SubscribedSnapshot<Vec<KernelEntry>>,
    pub(super) service_errors: SubscribedSnapshot<Vec<ServiceErrorEntry>>,
    pub(super) coredumps: SubscribedSnapshot<Vec<CoredumpEntry>>,
    pub(super) oom_kills: SubscribedSnapshot<Vec<OomKillEntry>>,
    pub(super) expansion: IndicatorGroupExpansion,
}

impl JournalScopeGroup {
    const fn family(&self) -> IndicatorGroupFamily {
        match self.unit_scope {
            SystemdUnitScope::SystemWide => IndicatorGroupFamily::SystemJournal,
            SystemdUnitScope::UserSession => IndicatorGroupFamily::UserJournal,
        }
    }

    pub(super) fn group_content<'view>(
        &'view self,
        config: &'view Config,
        now: SystemTime,
    ) -> IndicatorGroupContent<'view> {
        let unit_scope = self.unit_scope;
        let indicator_listing = [
            self.kernel_log_listing(config, now),
            self.service_errors_listing(config, now),
            self.coredump_log_listing(config, now),
            self.oom_kill_log_listing(config, now),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable(
            self.family(),
            GroupIndicator::glyph_only(self.family().title(), self.icon(), self.state(now)),
            &self.expansion,
            move |expansion_message| {
                JournalEventsInteraction::Expansion(unit_scope, expansion_message)
            },
            indicator_listing,
        )
    }

    fn kernel_log_listing<'view>(
        &'view self,
        config: &'view Config,
        now: SystemTime,
    ) -> IndicatorListing<'view> {
        self.kernel.indicator_listing(
            |held_entries| {
                self.wired_kernel_log_indicator(
                    config,
                    kernel_log_indicator(live_kernel_occurrences(held_entries, now)),
                    now,
                )
                .when_shown()
            },
            KERNEL_LOG_GLYPH.into(),
        )
    }

    fn service_errors_listing<'view>(
        &'view self,
        config: &'view Config,
        now: SystemTime,
    ) -> IndicatorListing<'view> {
        self.service_errors.indicator_listing(
            |held_entries| {
                let live_errors = live_service_errors(held_entries, now);
                let service_errors_row = service_errors_indicator(live_errors);
                self.wired_service_errors_indicator(service_errors_row, now)
                    .on_press(
                        TerminalCommand::JOURNAL_PRESS_HINT,
                        JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                            config,
                            live_service_errors(held_entries, now).follow_log_args(),
                        )),
                    )
                    .when_shown()
            },
            SERVICE_ERRORS_GLYPH.into(),
        )
    }

    fn coredump_log_listing<'view>(
        &'view self,
        config: &'view Config,
        now: SystemTime,
    ) -> IndicatorListing<'view> {
        self.coredumps.indicator_listing(
            |held_entries| {
                self.wired_coredump_log_indicator(
                    config,
                    coredump_log_indicator(live_coredumps(held_entries, now)),
                    now,
                )
                .when_shown()
            },
            COREDUMP_LOG_GLYPH.into(),
        )
    }

    fn oom_kill_log_listing<'view>(
        &'view self,
        config: &'view Config,
        now: SystemTime,
    ) -> IndicatorListing<'view> {
        self.oom_kills.indicator_listing(
            |held_entries| {
                self.wired_oom_kill_log_indicator(
                    config,
                    oom_kill_log_indicator(live_oom_kills(held_entries, now)),
                    now,
                )
                .when_shown()
            },
            OOM_KILL_LOG_GLYPH.into(),
        )
    }

    fn journal_kind_states(&self, now: SystemTime) -> [Option<IndicatorState>; 4] {
        [
            self.kernel.shown_state(
                |held_entries| kernel_log_state(&live_kernel_occurrences(held_entries, now)),
                |held_entries| {
                    kernel_log_panel_visibility(&live_kernel_occurrences(held_entries, now))
                },
            ),
            self.service_errors.shown_state(
                |held_entries| service_errors_state(&live_service_errors(held_entries, now)),
                |held_entries| {
                    service_errors_panel_visibility(&live_service_errors(held_entries, now))
                },
            ),
            self.coredumps.shown_state(
                |held_entries| coredump_log_state(&live_coredumps(held_entries, now)),
                |held_entries| coredump_log_panel_visibility(&live_coredumps(held_entries, now)),
            ),
            self.oom_kills.shown_state(
                |held_entries| oom_kill_log_state(&live_oom_kills(held_entries, now)),
                |held_entries| oom_kill_log_panel_visibility(&live_oom_kills(held_entries, now)),
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

    fn state(&self, now: SystemTime) -> IndicatorState {
        self.journal_kind_states(now)
            .into_iter()
            .flatten()
            .map(IndicatorState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
