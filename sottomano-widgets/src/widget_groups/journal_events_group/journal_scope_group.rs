use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelOccurrence,
    journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrors,
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::reading_display_state::ReadingDisplayState,
    widget_groups::{
        journal_events_group::{
            JournalEventsInteraction,
            coredump_log::{coredump_log_panel_visibility, coredump_log_state},
            kernel_error_log::{kernel_log_panel_visibility, kernel_log_state},
            oom_kill_log::{oom_kill_log_panel_visibility, oom_kill_log_state},
            service_error_log::{service_errors_panel_visibility, service_errors_state},
        },
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion, WidgetGroupFace},
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
    ) -> WidgetGroupContent<'a, JournalEventsInteraction> {
        let unit_scope = self.unit_scope;
        WidgetGroupContent::expandable(
            self.family(),
            &self.expansion,
            move |expansion_message| {
                JournalEventsInteraction::Expansion(unit_scope, expansion_message)
            },
            [
                self.kernel_log_item(config, clock),
                self.service_errors_item(config, clock),
                self.coredump_log_item(config, clock),
                self.oom_kill_log_item(config, clock),
            ]
            .into_iter()
            .flatten(),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: self.icon(),
            state: self.state(clock),
        })
    }

    fn journal_kind_states(&self, clock: Clock) -> [Option<ReadingDisplayState>; 4] {
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

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.journal_kind_states(clock)
            .into_iter()
            .flatten()
            .map(ReadingDisplayState::severity)
            .max()
            .unwrap_or_default()
            .into()
    }
}
