use chrono::{DateTime, Utc};
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, LAST_DAY,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct FocusRunReading<'a> {
    pub accounting: Accounting<'a>,
}

impl FocusRunReading<'_> {
    fn focused_now(&self, now: DateTime<Utc>) -> Section {
        let units = self.accounting.ledger.focused_units();
        let project = units.iter().find_map(|unit| match unit {
            FocusUnit::Project(root) => Some(root.clone()),
            FocusUnit::App(_) => None,
        });
        let app = units.iter().find_map(|unit| match unit {
            FocusUnit::App(app_id) => Some(app_id.clone()),
            FocusUnit::Project(_) => None,
        });
        Section::new(
            SectionTitle::captioned(
                "Focused now".to_owned(),
                "A run carries across window switches that share the project or app, and restarts \
                 after a break."
                    .to_owned(),
            ),
            dictionary_table(
                [
                    project.map(|root| LabeledValue::new("Focused project", ValueCell::Path(root))),
                    app.map(|app_id| LabeledValue::new("Focused app", app_id)),
                ]
                .into_iter()
                .flatten()
                .chain(self.accounting.runs(now).into_iter().map(|run| {
                    LabeledValue::new(
                        format!("On this {} since", run.unit.kind_label()),
                        ValueCell::Time {
                            at: run.since,
                            format: LocalizedTimeFormat::TimeOfDay,
                        },
                    )
                })),
            ),
        )
    }

    fn longest_session(&self, now: DateTime<Utc>) -> Option<Section> {
        let session = self
            .accounting
            .longest_session_within(LAST_DAY.ending_at(now), now)?;
        Some(Section::new(
            SectionTitle::captioned(
                "Last 24 hours".to_owned(),
                "A session runs from the end of one break to the start of the next, the same \
                 count as 'Active since last break'."
                    .to_owned(),
            ),
            dictionary_table([
                LabeledValue::new("Longest session", ValueCell::duration(session.length())),
                LabeledValue::new(
                    "That session started at",
                    ValueCell::Time {
                        at: session.from,
                        format: LocalizedTimeFormat::HourAndMinute,
                    },
                ),
            ]),
        ))
    }

    pub fn sections(&self, now: DateTime<Utc>) -> Vec<Section> {
        [Some(self.focused_now(now)), self.longest_session(now)]
            .into_iter()
            .flatten()
            .collect()
    }
}

impl<'a> FocusRunReading<'a> {
    pub fn reading_row(self, clock: Clock) -> ReadingRow<'a> {
        let label = match self.accounting.dominant_run(clock.utc()) {
            Some(run) => ReadoutLabel::Labeled(run.unit.label().into_owned()),
            None => ReadoutLabel::Unlabeled,
        };
        ReadingRow {
            readout: RowReadout {
                label,
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "Focus run",
                Phosphor::Target(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |card_clock| self.hover_card(card_clock)),
            )
        }
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections(clock.utc()).into_iter().collect::<Vec<_>>()
    }
}
