use std::{convert::Infallible, time::Duration};

use chrono::{DateTime, Datelike, LocalResult, Locale, NaiveTime, Offset, TimeZone, Utc};
use iced::futures::{Stream, stream};
use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate;

impl SnapshotSource for LocalDate {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Local date";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::unfold((), |()| async {
            sleep(until_next_midnight(&Clock::real().local())).await;
            Some((SnapshotDelivery::live_push(Ok(LocalDate)), ()))
        })
    }
}

fn until_next_midnight<Zone: TimeZone>(now: &DateTime<Zone>) -> Duration {
    let next_day_start = now
        .date_naive()
        .succ_opt()
        .expect("the calendar has a day after today")
        .and_time(NaiveTime::MIN);
    let next_midnight = match now.timezone().from_local_datetime(&next_day_start) {
        LocalResult::Single(midnight) | LocalResult::Ambiguous(midnight, _) => {
            midnight.with_timezone(&Utc)
        }
        LocalResult::None => now
            .offset()
            .fix()
            .from_local_datetime(&next_day_start)
            .single()
            .expect("a fixed offset maps every local time to one instant")
            .with_timezone(&Utc),
    };
    (next_midnight - now.with_timezone(&Utc))
        .to_std()
        .expect("the next midnight lies after now")
}

impl LocalDate {
    pub(super) fn reading_row(clock: Clock, locale: Locale) -> ReadingRow<'static> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(
                    LocalizedTimeFormat::Date.format(&clock.local(), locale),
                ),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                Self::NAME,
                Phosphor::CalendarBlank(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(Self::hover_card),
            )
        }
    }

    pub(super) fn hover_card(clock: Clock) -> Vec<Section> {
        let local = clock.local();
        vec![Section::new(
            "Calendar",
            dictionary_table([
                LabeledValue::new(
                    "Weekday",
                    ValueCell::Time {
                        at: local.with_timezone(&Utc),
                        format: LocalizedTimeFormat::Weekday,
                    },
                ),
                LabeledValue::new("Week", local.iso_week().week().to_string()),
                LabeledValue::new("Day of year", local.ordinal().to_string()),
            ]),
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::TimeZone;
    use chrono_tz::{
        America::{Havana, Santiago},
        Europe::Brussels,
    };

    use super::until_next_midnight;

    #[test]
    fn an_ordinary_evening_waits_until_midnight() {
        let evening = Brussels.with_ymd_and_hms(2026, 11, 16, 18, 30, 0).unwrap();
        assert_eq!(
            until_next_midnight(&evening),
            Duration::from_mins(5 * 60 + 30)
        );
    }

    #[test]
    fn a_skipped_midnight_waits_until_the_clocks_jump() {
        let evening = Santiago.with_ymd_and_hms(2026, 9, 5, 23, 0, 0).unwrap();
        assert_eq!(until_next_midnight(&evening), Duration::from_hours(1));
    }

    #[test]
    fn a_repeated_midnight_waits_until_its_first_occurrence() {
        let evening = Havana.with_ymd_and_hms(2026, 10, 31, 22, 0, 0).unwrap();
        assert_eq!(until_next_midnight(&evening), Duration::from_hours(2));
    }
}
