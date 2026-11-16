use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Locale, NaiveDate};
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    caldav_tasks_events::{
        agenda_moment_zones::AgendaMoment,
        caldav_event_query::{EventPhase, UpcomingEvent},
        caldav_server_provider::CaldavProvider,
        caldav_upcoming_events::UpcomingEventList,
        nextcloud_web_links::NextcloudWebRoot,
    },
};

use crate::{
    clock::Clock,
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{
        Section, SectionTitle, SectionWidget, dictionary_table, headed_grouped_table,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
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

impl From<&EventRow<'_>> for TableRecord<4> {
    fn from(event_row: &EventRow<'_>) -> Self {
        let details_link = event_row
            .provider
            .nextcloud_web_root()
            .and_then(|nextcloud| nextcloud.event_details(event_row.event));
        let title = match details_link {
            Some(url) => linked_title(&event_row.event.summary, url),
            None => event_row.event.summary.as_str().into(),
        };
        TableRecord::with_cells(
            title,
            [
                event_date_cell(event_row.event.start),
                event_time_cell(event_row.event.start),
                ValueCell::Hint(event_row.event.calendar.clone()),
                if event_row.event.location.is_empty() {
                    ValueCell::Blank
                } else {
                    ValueCell::from(event_row.event.location.clone())
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
    match start {
        AgendaMoment::AllDay(_) => ValueCell::Blank,
        AgendaMoment::Floating(_) | AgendaMoment::Zoned(_) => ValueCell::Time {
            at: start.starts_at().to_utc(),
            format: LocalizedTimeFormat::HourAndMinute,
        },
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

pub(super) fn nextcloud_app_link(
    app_label: &str,
    provider: &CaldavProvider,
    app_url: fn(&NextcloudWebRoot) -> String,
) -> Option<LabeledValue> {
    provider.nextcloud_web_root().map(|nextcloud| {
        LabeledValue::new(
            "Nextcloud",
            ValueCell::BrandedLink {
                label: app_label.to_owned(),
                url: app_url(nextcloud),
                glyph: SimpleIcon::Nextcloud.into(),
            },
        )
    })
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
    if day_offset == 0 && !matches!(start, AgendaMoment::AllDay(_)) {
        return LocalizedTimeFormat::HourAndMinute.format(&begins, locale);
    }
    relative_day_word(day_offset, locale)
        .unwrap_or_else(|| LocalizedTimeFormat::AbbreviatedWeekday.format(&begins, locale))
}

pub(crate) const UPCOMING_EVENT_LIST_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::CalendarDots(Weight::Regular));

pub(crate) fn upcoming_events_indicator(
    upcoming_events: &UpcomingEventList,
    clock: Clock,
    locale: Locale,
) -> ReadingRow<'_> {
    let now = clock.local();
    let label = match upcoming_events.next_event(now) {
        Some(event) => {
            let summary: String = event.summary.chars().take(LABEL_CHARACTERS).collect();
            format!("{}: {summary}", starts_label(event.start, now, locale))
        }
        None => empty_agenda_label(&upcoming_events.failed_subscription_names),
    };
    let state = if upcoming_events
        .events
        .iter()
        .any(|event| event.phase(now) == EventPhase::Ongoing)
    {
        IndicatorState::On
    } else if upcoming_events.failed_subscription_names.is_empty() {
        IndicatorState::Idle
    } else {
        IndicatorState::Warning
    };
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Labeled(label),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            UpcomingEventList::NAME,
            UPCOMING_EVENT_LIST_FALLBACK_GLYPH,
            state,
            Box::new(move |card_clock| upcoming_events_hover_card(upcoming_events, card_clock)),
        )
    }
}

pub(crate) fn upcoming_events_hover_card(
    upcoming_events: &UpcomingEventList,
    clock: Clock,
) -> Vec<Section> {
    let now = clock.local();
    let rows: Vec<_> = upcoming_events
        .events
        .iter()
        .filter(|event| event.phase(now) != EventPhase::Ended)
        .map(|event| EventRow {
            event,
            provider: &upcoming_events.provider,
        })
        .collect();
    let failed_subscription_sections =
        upcoming_events
            .failed_subscription_names
            .iter()
            .map(|calendar_name| {
                Section::new(
                    "Error",
                    SectionWidget::paragraph(format!(
                        "calendar {calendar_name} could not be downloaded"
                    )),
                )
            });
    failed_subscription_sections
        .chain([
            Section::new(
                SectionTitle::captioned(
                    "Agenda",
                    calendar_count_caption(upcoming_events.calendar_count),
                ),
                headed_grouped_table(
                    [
                        ColumnHeading::titled("Date"),
                        ColumnHeading::titled("Time"),
                        ColumnHeading::titled("Calendar").align_x(Horizontal::Left),
                        ColumnHeading::titled("Location").align_x(Horizontal::Left),
                    ],
                    rows.chunk_by(|earlier, later| earlier.starts_on() == later.starts_on()),
                )
                .truncated_to(LISTED_EVENTS),
            ),
            Section::new(
                "Links",
                dictionary_table(nextcloud_app_link(
                    "open Calendar",
                    &upcoming_events.provider,
                    NextcloudWebRoot::calendar_app,
                )),
            ),
        ])
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Locale;
    use sottomano_desktop_sources::{
        SnapshotSource,
        caldav_tasks_events::{
            caldav_server_provider::CaldavProvider, caldav_upcoming_events::UpcomingEventList,
        },
        network_connectivity::internet_connectivity_gate::InternetUnreachable,
    };

    use crate::{
        clock::Clock,
        group_indicators::{
            indicator_drawing::ReadoutLabel,
            indicator_state::IndicatorState,
            offline_internet_indicator::{PlaceholderCause, SnapshotPlaceholder, SnapshotTrouble},
        },
        widget_groups::caldav_group::upcoming_events_indicator::{
            UPCOMING_EVENT_LIST_FALLBACK_GLYPH, upcoming_events_indicator,
        },
        widgets::subscribed_snapshot::SubscribedSnapshot,
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
    fn events_awaiting_their_first_snapshot_show_a_pending_indicator() {
        let pending_events = SubscribedSnapshot::<UpcomingEventList>::default();
        assert!(!pending_events.hover_card_with(Vec::new()).is_empty());
        assert!(matches!(
            pending_events,
            SubscribedSnapshot::AwaitingFirstSnapshot
        ));
        assert_eq!(
            SnapshotPlaceholder::<UpcomingEventList>::awaiting_row(
                UPCOMING_EVENT_LIST_FALLBACK_GLYPH
            )
            .shown_state(),
            IndicatorState::Pending
        );
    }

    fn event_placeholder(
        placeholder_cause: PlaceholderCause<<UpcomingEventList as SnapshotSource>::Error>,
    ) -> SnapshotPlaceholder<UpcomingEventList> {
        SnapshotPlaceholder { placeholder_cause }
    }

    #[test]
    fn a_pending_indicator_draws_no_label_until_its_data_arrives() {
        let pending_events = event_placeholder(PlaceholderCause::AwaitingFirstSnapshot);
        let pending_row = pending_events.reading_row(UPCOMING_EVENT_LIST_FALLBACK_GLYPH);
        assert_eq!(pending_row.readout.label, ReadoutLabel::Unlabeled);
        assert_eq!(pending_row.shown_state(), IndicatorState::Pending);
        let card_titles: Vec<String> = pending_row
            .hover_card(Clock::real())
            .iter()
            .map(|section| section.title().text().to_owned())
            .collect();
        assert_eq!(card_titles, ["Loading".to_owned()]);
    }

    #[test]
    fn an_offline_placeholder_is_not_reported_as_pending() {
        let offline_events = event_placeholder(PlaceholderCause::Troubled(
            SnapshotTrouble::Offline(InternetUnreachable::NoConnection),
        ));
        let offline_row = offline_events.reading_row(UPCOMING_EVENT_LIST_FALLBACK_GLYPH);
        assert_eq!(offline_row.readout.label, ReadoutLabel::Unlabeled);
        assert_eq!(offline_row.shown_state(), IndicatorState::Offline);
    }

    #[test]
    fn a_free_agenda_keeps_a_visible_indicator() {
        let event_list = event_list_without_events(Vec::new());
        let row = upcoming_events_indicator(&event_list, Clock::real(), Locale::en_US);
        assert_eq!(
            row.readout.label,
            ReadoutLabel::Labeled("No upcoming events".to_owned()),
        );
        assert_eq!(row.shown_state(), IndicatorState::Idle);
    }

    #[test]
    fn a_failed_subscription_is_not_reported_as_a_free_agenda() {
        let event_list = event_list_without_events(vec!["Proton".to_owned()]);
        let row = upcoming_events_indicator(&event_list, Clock::real(), Locale::en_US);
        assert_eq!(
            row.readout.label,
            ReadoutLabel::Labeled("Proton unavailable".to_owned()),
        );
        assert_eq!(row.shown_state(), IndicatorState::Warning);
    }
}
