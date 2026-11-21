use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelOccurrence,
    journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrors,
};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
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
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub struct JournalScopeGroup {
    pub(super) unit_scope: SystemdUnitScope,
    pub(super) kernel: SubscribedSnapshot<Vec<KernelOccurrence>>,
    pub(super) service_errors: SubscribedSnapshot<ServiceErrors>,
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
    ) -> IndicatorGroupContent<'view> {
        let unit_scope = self.unit_scope;
        let mut rows = Vec::new();
        self.push_kernel_log_indicators(&mut rows, config);
        self.push_service_errors_indicators(&mut rows, config);
        self.push_coredump_log_indicators(&mut rows, config);
        self.push_oom_kill_log_indicators(&mut rows, config);
        IndicatorGroupContent::expandable(
            self.family(),
            GroupIndicator::glyph_only(self.family().title(), self.icon(), self.state()),
            &self.expansion,
            move |expansion_message| {
                JournalEventsInteraction::Expansion(unit_scope, expansion_message)
            },
            rows,
        )
    }

    fn push_kernel_log_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        config: &'view Config,
    ) {
        if !self.kernel.awaits_first_delivery() {
            let kernel_indicator = self.kernel.indicator(
                |held_snapshot| kernel_log_indicator(held_snapshot),
                KERNEL_LOG_GLYPH.into(),
            );
            if kernel_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_kernel_log_indicator(config, kernel_indicator));
            }
        }
    }

    fn push_service_errors_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        config: &'view Config,
    ) {
        if !self.service_errors.awaits_first_delivery() {
            let service_errors_row = self.wired_service_errors_indicator(
                self.service_errors
                    .indicator(service_errors_indicator, SERVICE_ERRORS_GLYPH.into()),
            );
            if service_errors_row.panel_visibility == PanelVisibility::Shown {
                rows.push(match self.service_errors.get() {
                    Some(held_snapshot) => service_errors_row.on_press(
                        TerminalCommand::JOURNAL_PRESS_HINT,
                        JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                            config,
                            held_snapshot.follow_log_args(),
                        )),
                    ),
                    None => service_errors_row,
                });
            }
        }
    }

    fn push_coredump_log_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        config: &'view Config,
    ) {
        if !self.coredumps.awaits_first_delivery() {
            let coredump_indicator = self.coredumps.indicator(
                |held_snapshot| coredump_log_indicator(held_snapshot),
                COREDUMP_LOG_GLYPH.into(),
            );
            if coredump_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_coredump_log_indicator(config, coredump_indicator));
            }
        }
    }

    fn push_oom_kill_log_indicators<'view>(
        &'view self,
        rows: &mut Vec<GroupIndicator<'view>>,
        config: &'view Config,
    ) {
        if !self.oom_kills.awaits_first_delivery() {
            let oom_kill_indicator = self.oom_kills.indicator(
                |held_snapshot| oom_kill_log_indicator(held_snapshot),
                OOM_KILL_LOG_GLYPH.into(),
            );
            if oom_kill_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_oom_kill_log_indicator(config, oom_kill_indicator));
            }
        }
    }

    fn journal_kind_states(&self) -> [Option<IndicatorState>; 4] {
        [
            self.kernel.shown_state(
                |snapshot| kernel_log_state(snapshot),
                |snapshot| kernel_log_panel_visibility(snapshot),
            ),
            self.service_errors
                .shown_state(service_errors_state, service_errors_panel_visibility),
            self.coredumps.shown_state(
                |snapshot| coredump_log_state(snapshot),
                |snapshot| coredump_log_panel_visibility(snapshot),
            ),
            self.oom_kills.shown_state(
                |snapshot| oom_kill_log_state(snapshot),
                |snapshot| oom_kill_log_panel_visibility(snapshot),
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

    fn state(&self) -> IndicatorState {
        self.journal_kind_states()
            .into_iter()
            .flatten()
            .map(IndicatorState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
