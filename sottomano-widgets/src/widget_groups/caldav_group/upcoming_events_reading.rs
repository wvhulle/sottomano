use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Locale, NaiveDate};
use iced::alignment::Horizontal;
use sottomano_sources::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment, caldav_event_query::UpcomingEvent,
    caldav_server_provider::CaldavProvider, caldav_upcoming_events::UpcomingEventList,
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_durations::{relative_day_word, relative_moment_text},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

const LISTED_EVENTS: NonZeroUsize = NonZeroUsize::new(10).unwrap();
const LABEL_CHARACTERS: usize = 28;

struct EventRow<'a> {
    event: &'a UpcomingEvent,
    provider: &'a CaldavProvider,
}

impl EventRow<'_> {
    fn starts_on(&self) -> NaiveDate {
        self.event.start.starts_at().date_naive()
    }
}

impl IntoTableRecord<4> for &EventRow<'_> {
    fn into_table_record(self) -> TableRecord<4> {
        let details_link = self
            .provider
            .nextcloud_web_root()
            .and_then(|nextcloud| nextcloud.event_details(self.event));
        let title = match details_link {
            Some(url) => linked_title(&self.event.summary, url),
            None => self.event.summary.as_str().into(),
        };
        TableRecord::new(
            title,
            [
                event_date_cell(self.event.start),
                event_time_cell(self.event.start),
                ValueCell::Hint(self.event.calendar.clone()),
                if self.event.location.is_empty() {
                    ValueCell::Blank
                } else {
                    ValueCell::from(self.event.location.clone())
                },
            ],
        )
    }
}

pub(super) fn event_date_cell(start: AgendaMoment) -> ValueCell {
    ValueCell::Time {
        at: start.starts_at().to_utc(),
        format: LocalizedTimeFormat::WeekdayDate,
    }
}

pub(super) fn event_time_cell(start: AgendaMoment) -> ValueCell {
    if start.is_all_day() {
        return ValueCell::Blank;
    }
    ValueCell::Time {
        at: start.starts_at().to_utc(),
        format: LocalizedTimeFormat::HourAndMinute,
    }
}

pub(super) fn linked_title(title: &str, url: String) -> ValueCell {
    ValueCell::Link {
        label: title.to_owned(),
        url,
    }
}

fn calendar_count_caption(calendar_count: usize) -> String {
    format!("{calendar_count} calendars over CalDAV.")
}

pub(super) fn nextcloud_footer(
    app_label: &str,
    provider: &CaldavProvider,
    app_url: fn(&NextcloudWebRoot) -> String,
) -> Section {
    Section::dictionary(provider.nextcloud_web_root().map(|nextcloud| {
        LabeledValue::new(
            "Nextcloud",
            ValueCell::BrandedLink {
                label: app_label.to_owned(),
                url: app_url(nextcloud),
                glyph: SimpleIcon::Nextcloud.into(),
            },
        )
    }))
}

fn empty_agenda_label(failed_subscription_names: &[String]) -> String {
    if failed_subscription_names.is_empty() {
        "No upcoming events".to_owned()
    } else {
        format!("{} unavailable", failed_subscription_names.join(", "))
    }
}

fn starts_label(start: AgendaMoment, now: DateTime<Local>, locale: Locale) -> String {
    let begins = start.starts_at();
    if begins <= now {
        return relative_moment_text(now.to_utc(), now.to_utc(), locale);
    }
    let day_offset = (begins.date_naive() - now.date_naive()).num_days();
    if day_offset == 0 && !start.is_all_day() {
        return LocalizedTimeFormat::HourAndMinute.format(&begins, locale);
    }
    relative_day_word(day_offset, locale)
        .unwrap_or_else(|| LocalizedTimeFormat::AbbreviatedWeekday.format(&begins, locale))
}

impl GroupItemReading for UpcomingEventList {
    fn name(&self) -> &'static str {
        "Upcoming events"
    }

    fn icon(&self) -> IconGlyph {
        Self::fallback_icon()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        let now = clock.local();
        let Some(event) = self.next_event(now) else {
            return Some(empty_agenda_label(&self.failed_subscription_names));
        };
        let summary: String = event.summary.chars().take(LABEL_CHARACTERS).collect();
        Some(format!(
            "{}: {summary}",
            starts_label(event.start, now, locale)
        ))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        let now = clock.local();
        if self.events.iter().any(|event| event.is_ongoing(now)) {
            ReadingDisplayState::On
        } else if self.failed_subscription_names.is_empty() {
            ReadingDisplayState::Idle
        } else {
            ReadingDisplayState::Warning
        }
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let now = clock.local();
        let rows: Vec<_> = self
            .events
            .iter()
            .filter(|event| !event.has_ended(now))
            .map(|event| EventRow {
                event,
                provider: &self.provider,
            })
            .collect();
        let failed_subscription_sections =
            self.failed_subscription_names.iter().map(|calendar_name| {
                Section::error(format!("calendar {calendar_name} could not be downloaded"))
            });
        HoverCardBody::sections(
            failed_subscription_sections.chain([
                Section::grouped_column_table(
                    [
                        ColumnHeading::titled("Date"),
                        ColumnHeading::titled("Time"),
                        ColumnHeading::titled("Calendar").align_x(Horizontal::Left),
                        ColumnHeading::titled("Location").align_x(Horizontal::Left),
                    ],
                    rows.chunk_by(|earlier, later| earlier.starts_on() == later.starts_on()),
                )
                .truncated_to(LISTED_EVENTS)
                .titled((
                    "Upcoming events",
                    calendar_count_caption(self.calendar_count),
                )),
                nextcloud_footer(
                    "open Calendar",
                    &self.provider,
                    NextcloudWebRoot::calendar_app,
                ),
            ]),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use chrono::Locale;
    use sottomano_sources::{
        caldav_tasks_events::{
            caldav_server_provider::CaldavProvider, caldav_upcoming_events::UpcomingEventList,
        },
        network_connectivity::internet_connectivity_gate::InternetUnreachable,
    };

    use crate::{
        clock::Clock,
        group_items::{
            group_item_reading::GroupItemReading,
            offline_internet_reading::{PlaceholderCause, SnapshotPlaceholder},
            reading_display_state::ReadingDisplayState,
        },
        hover_cards::hover_card_body::{HoverCardBody, Section},
        widget_groups::caldav_group::CaldavTarget,
        widgets::latest_snapshot::LatestSnapshot,
    };

    fn event_list_without_events(failed_subscription_names: Vec<String>) -> UpcomingEventList {
        UpcomingEventList {
            events: Vec::new(),
            calendar_count: 1,
            failed_subscription_names,
            provider: CaldavProvider::Unrecognised,
        }
    }

    #[test]
    fn events_awaiting_their_first_snapshot_show_a_pending_reading() {
        let pending_events = LatestSnapshot::<UpcomingEventList>::default();
        assert!(pending_events.hover_card(Clock::real()).is_none());
        assert!(
            pending_events
                .hover_card_while_pending(Clock::real())
                .is_some()
        );
        assert!(
            pending_events
                .group_item_while_pending::<()>(CaldavTarget::UpcomingEvents)
                .is_some()
        );
    }

    fn event_placeholder(
        placeholder_cause: PlaceholderCause,
    ) -> SnapshotPlaceholder<UpcomingEventList> {
        SnapshotPlaceholder {
            placeholder_cause,
            placeholder_source: PhantomData,
        }
    }

    #[test]
    fn a_pending_reading_draws_no_label_until_its_data_arrives() {
        let pending_events = event_placeholder(PlaceholderCause::AwaitingFirstSnapshot);
        assert_eq!(pending_events.label(Clock::real(), Locale::en_US), None);
        assert_eq!(
            pending_events.state(Clock::real()),
            ReadingDisplayState::Pending,
        );
        assert_eq!(
            pending_events.hover_card(Clock::real()),
            HoverCardBody::sections([Section::section_seeding()]),
        );
    }

    #[test]
    fn an_offline_placeholder_is_not_reported_as_pending() {
        let offline_events =
            event_placeholder(PlaceholderCause::Offline(InternetUnreachable::NoConnection));
        assert_eq!(offline_events.label(Clock::real(), Locale::en_US), None);
        assert_eq!(
            offline_events.state(Clock::real()),
            ReadingDisplayState::Off
        );
    }

    #[test]
    fn a_free_agenda_keeps_a_visible_reading() {
        let event_list = event_list_without_events(Vec::new());
        assert_eq!(
            event_list.label(Clock::real(), Locale::en_US).as_deref(),
            Some("No upcoming events"),
        );
        assert_eq!(event_list.state(Clock::real()), ReadingDisplayState::Idle);
    }

    #[test]
    fn a_failed_subscription_is_not_reported_as_a_free_agenda() {
        let event_list = event_list_without_events(vec!["Proton".to_owned()]);
        assert_eq!(
            event_list.label(Clock::real(), Locale::en_US).as_deref(),
            Some("Proton unavailable"),
        );
        assert_eq!(
            event_list.state(Clock::real()),
            ReadingDisplayState::Warning
        );
    }
}
