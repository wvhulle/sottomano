use std::{convert::Infallible, time::Duration};

use chrono::{DateTime, Datelike, LocalResult, Locale, NaiveTime, Offset, TimeZone, Utc};
use iced::futures::{Stream, StreamExt, stream};
use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};
use tokio::time::sleep;

use super::{ClockGroup, ClockInteraction, ClockSource};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
    },
};

const LOCAL_DATE_GLYPH: Phosphor = Phosphor::CalendarBlank(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate;

impl SnapshotSource for LocalDate {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Local date";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(async { SnapshotDelivery::live_push(Ok(LocalDate)) }).chain(stream::unfold(
            (),
            |()| async {
                sleep(until_next_midnight(&Clock::real().local())).await;
                Some((SnapshotDelivery::live_push(Ok(LocalDate)), ()))
            },
        ))
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
    pub(super) fn indicator<'indicator>(
        clock: Clock,
        locale: Locale,
    ) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            Self::NAME,
            Phosphor::CalendarBlank(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(LocalizedTimeFormat::Date.format(&clock.local(), locale)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(Self::hover_card))
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

impl ClockGroup {
    pub(super) fn local_date_listing(&self, clock: Clock, locale: Locale) -> IndicatorListing<'_> {
        self.date
            .indicator_listing(
                |_| LocalDate::indicator(clock, locale).when_shown(),
                LOCAL_DATE_GLYPH.into(),
            )
            .map_indicators(|date_row| self.wired_local_date_indicator(date_row))
    }

    fn wired_local_date_indicator<'view>(
        &'view self,
        date_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        date_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.date
                    .sections_described_by(|_| LocalDate::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalDate))
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
