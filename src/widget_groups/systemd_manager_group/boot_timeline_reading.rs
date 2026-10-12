use std::time::Duration;

use crate::{
    data::source::systemd_manager::boot_timeline::{
        BootPhase, BootTimeline, RankedActivations, UnitActivation,
    },
    widget_groups::systemd_manager_group::elapsed_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl UnitActivation {
    fn key_value(&self) -> KeyValue {
        KeyValue::new(self.name.clone(), elapsed_text(self.duration)).level(self.severity)
    }
}

impl BootPhase {
    fn table_record(self) -> TableRecord<2> {
        TableRecord::new(
            self.stage.name(),
            [
                Some(elapsed_text(self.duration).into()),
                Some(Cell::Hint(self.stage.meaning().to_owned())),
            ],
        )
    }
}

fn timeline_record(stage: &str, took: Duration, meaning: &str) -> TableRecord<2> {
    TableRecord::new(
        stage.to_owned(),
        [
            Some(elapsed_text(took).into()),
            Some(Cell::Hint(meaning.to_owned())),
        ],
    )
}

impl BootTimeline {
    fn timeline_records(&self) -> impl Iterator<Item = TableRecord<2>> {
        let login_gap = self
            .user_startup
            .as_ref()
            .map(|user| user.started_at.saturating_sub(self.system_startup.finish));
        let session = self.user_startup.as_ref().map(|user| user.duration);
        self.phases
            .present()
            .into_iter()
            .map(BootPhase::table_record)
            .chain(login_gap.map(|gap| {
                timeline_record("login", gap, "waiting for the first user session manager")
            }))
            .chain(session.map(|duration| {
                timeline_record(
                    "user session",
                    duration,
                    "user systemd starting units until the default target",
                )
            }))
    }

    pub fn units(&self) -> StartupUnits<'_> {
        StartupUnits { timeline: self }
    }
}

impl BarWidget for BootTimeline {
    fn name(&self) -> &'static str {
        "Boot timeline"
    }

    fn icon(&self) -> Glyph {
        Phosphor::RocketLaunch(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(elapsed_text(self.phases.total()))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            ["Took", "Meaning"],
            self.timeline_records(),
        )])
    }
}

pub struct StartupUnits<'a> {
    timeline: &'a BootTimeline,
}

impl BarWidget for StartupUnits<'_> {
    fn name(&self) -> &'static str {
        "Startup units"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Stack(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        let count = self
            .timeline
            .system_startup
            .slowest_units
            .slow_count()
            .saturating_add(
                self.timeline
                    .user_startup
                    .as_ref()
                    .map_or(0, |user| user.slowest_units.slow_count()),
            );
        u32::try_from(count).ok().filter(|count| *count > 0)
    }

    fn state(&self) -> WidgetState {
        let user_state = self
            .timeline
            .user_startup
            .as_ref()
            .map(|user| user.slowest_units.slowness_state());
        [
            Some(self.timeline.system_startup.slowest_units.slowness_state()),
            user_state,
        ]
        .into_iter()
        .flatten()
        .map(WidgetState::severity)
        .max()
        .unwrap_or_default()
        .into()
    }

    fn tooltip(&self) -> TooltipBody {
        let user_section = self.timeline.user_startup.as_ref().map(|user| {
            user.slowest_units
                .blame_section("Slowest user units at login")
        });
        let sections: Vec<Section> = [
            Some(
                self.timeline
                    .system_startup
                    .slowest_units
                    .blame_section("Slowest system units at boot"),
            ),
            user_section,
        ]
        .into_iter()
        .flatten()
        .collect();
        TooltipBody::sections(sections)
    }
}

impl RankedActivations {
    fn slowness_state(&self) -> WidgetState {
        self.slowest_first
            .iter()
            .map(|activation| activation.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn blame_section(&self, title: &str) -> Section {
        Section::dictionary(self.slowest_first.iter().map(UnitActivation::key_value)).titled((
            title,
            "Time from starting to active, for units up before startup finished. Devices and \
             units under 0.1 s are left out; amber ones crossed slow_boot_unit_secs.",
        ))
    }
}
