use std::borrow::Cow;

use sottomano_desktop_sources::systemd_units_journal::{
    SystemdUnitScope, journal_coredumps::CoredumpEntry, journal_kernel_errors::KernelOccurrence,
    journal_oom_kills::OomKillEntry, journal_service_errors::ServiceErrors,
};
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::{
        journal_events_group::JournalEventsInteraction,
        widget_group_content::{WidgetGroupContent, WidgetGroupExpansion, WidgetGroupFace},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
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
                |snapshot| snapshot.state(clock),
                GroupItemReading::panel_visibility,
                clock,
            ),
            self.service_errors.shown_state(
                |snapshot| snapshot.state(clock),
                GroupItemReading::panel_visibility,
                clock,
            ),
            self.coredumps.shown_state(
                |snapshot| snapshot.state(clock),
                GroupItemReading::panel_visibility,
                clock,
            ),
            self.oom_kills.shown_state(
                |snapshot| snapshot.state(clock),
                GroupItemReading::panel_visibility,
                clock,
            ),
        ]
    }
}

impl GroupItemReading for JournalScopeGroup {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(self.family().title())
    }

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

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let kernel = self
            .kernel
            .visible_reading(GroupItemReading::panel_visibility)
            .map(|errors| LabeledValue::new("Kernel errors", Quantity::count(errors.len())));
        let service = self
            .service_errors
            .visible_reading(GroupItemReading::panel_visibility)
            .map(|errors| LabeledValue::new("Service errors", Quantity::count(errors.entry_count)));
        let coredumps = self
            .coredumps
            .visible_reading(GroupItemReading::panel_visibility)
            .map(|coredumps| LabeledValue::new("Coredumps", Quantity::count(coredumps.len())));
        let oom_kills = self
            .oom_kills
            .visible_reading(GroupItemReading::panel_visibility)
            .map(|kills| LabeledValue::new("OOM kills", Quantity::count(kills.len())));
        vec![Section::new(
            "Last hour",
            dictionary_table(
                kernel
                    .into_iter()
                    .chain(service)
                    .chain(coredumps)
                    .chain(oom_kills),
            ),
        )]
    }
}
