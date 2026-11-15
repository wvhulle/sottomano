use chrono::Locale;
use sottomano_desktop_sources::disk_storage::ext4_sysfs_counters::RecordedErrors;
use sottomano_toml_config::severity::Severity;

use crate::{
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const ERROR_MOMENT_FORMAT: &str = "%Y-%m-%d %H:%M UTC";

pub(super) struct Ext4ErrorCountRow {
    pub recorded_errors: RecordedErrors,
}

impl Ext4ErrorCountRow {
    pub(super) fn reading_row(self, locale: Locale) -> ReadingRow<'static> {
        let state = match self.recorded_errors {
            RecordedErrors::Clean => Severity::Normal,
            RecordedErrors::Recorded { .. } => Severity::Warning,
        };
        let badge = match self.recorded_errors {
            RecordedErrors::Clean => ReadoutBadge::Unbadged,
            RecordedErrors::Recorded { count, .. } => ReadoutBadge::Counted(
                usize::try_from(count)
                    .expect("usize holds every u64 on the 64-bit targets we build"),
            ),
        };
        let label = match self.recorded_errors {
            RecordedErrors::Clean => "none".to_owned(),
            RecordedErrors::Recorded { count, .. } => Quantity::count(count).localized_text(locale),
        };
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label),
                gauge: ReadoutGauge::Ungauged,
                badge,
            },
            ..ReadingRow::glyph_only(
                "Recorded errors",
                Phosphor::WarningCircle(Weight::Regular).into(),
                state.into(),
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        match self.recorded_errors {
            RecordedErrors::Clean => {
                vec![Section::new(
                    "History",
                    dictionary_table([LabeledValue::new("Recorded errors", "none")]),
                )]
            }
            RecordedErrors::Recorded {
                count,
                first_seen,
                last_seen,
            } => vec![Section::new(
                "History",
                dictionary_table([
                    LabeledValue::new("Recorded errors", Quantity::count(count))
                        .level(Severity::Warning),
                    LabeledValue::new(
                        "First seen",
                        first_seen.format(ERROR_MOMENT_FORMAT).to_string(),
                    ),
                    LabeledValue::new(
                        "Last seen",
                        last_seen.format(ERROR_MOMENT_FORMAT).to_string(),
                    ),
                ]),
            )],
        }
    }
}
