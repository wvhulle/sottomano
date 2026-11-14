use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::disk_storage::ext4_sysfs_counters::RecordedErrors;
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const ERROR_MOMENT_FORMAT: &str = "%Y-%m-%d %H:%M UTC";

pub(super) struct Ext4ErrorCountRow {
    pub recorded_errors: RecordedErrors,
}

impl GroupItemReading for Ext4ErrorCountRow {
    fn name(&self) -> Cow<'static, str> {
        "Recorded errors".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WarningCircle(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.recorded_errors {
            RecordedErrors::Clean => Severity::Normal,
            RecordedErrors::Recorded { .. } => Severity::Warning,
        }
        .into()
    }

    fn badge(&self) -> Option<usize> {
        match self.recorded_errors {
            RecordedErrors::Clean => None,
            RecordedErrors::Recorded { count, .. } => Some(
                usize::try_from(count)
                    .expect("usize holds every u64 on the 64-bit targets we build"),
            ),
        }
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(match self.recorded_errors {
            RecordedErrors::Clean => "none".to_owned(),
            RecordedErrors::Recorded { count, .. } => Quantity::count(count).localized_text(locale),
        })
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        match self.recorded_errors {
            RecordedErrors::Clean => {
                HoverCardBody::dictionary([LabeledValue::new("Recorded errors", "none")])
            }
            RecordedErrors::Recorded {
                count,
                first_seen,
                last_seen,
            } => HoverCardBody::dictionary([
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
        }
    }
}
