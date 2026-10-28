use std::num::NonZeroUsize;

use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_sources::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment, caldav_agenda::CaldavAgenda,
    caldav_event_query::UpcomingEvent, caldav_server_provider::CaldavProvider,
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_durations::{relative_day_word, relative_moment_text},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
        widget_state::WidgetState,
    },
};

const LISTED_EVENTS: NonZeroUsize = NonZeroUsize::new(10).unwrap();
const LABEL_CHARACTERS: usize = 28;

struct EventRow<'a> {
    event: &'a UpcomingEvent,
    provider: &'a CaldavProvider,
}

impl TableRow<4> for EventRow<'_> {
    fn table_record(self) -> TableRecord<4> {
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
                Some(event_date_cell(self.event.start)),
                event_time_cell(self.event.start),
                Some(Cell::Hint(self.event.calendar.clone())),
                (!self.event.location.is_empty()).then(|| Cell::from(self.event.location.clone())),
            ],
        )
    }
}

fn event_date_cell(start: AgendaMoment) -> Cell {
    Cell::Time {
        at: start.starts_at().to_utc(),
        format: LocalizedTimeFormat::WeekdayDate,
    }
}

fn event_time_cell(start: AgendaMoment) -> Option<Cell> {
    (!start.is_all_day()).then(|| Cell::Time {
        at: start.starts_at().to_utc(),
        format: LocalizedTimeFormat::HourAndMinute,
    })
}

pub(super) fn linked_title(title: &str, url: String) -> Cell {
    Cell::Link {
        label: title.to_owned(),
        url,
        glyph: None,
    }
}

pub(super) fn moment_cell(moment: AgendaMoment) -> Cell {
    Cell::Time {
        at: moment.starts_at().to_utc(),
        format: if moment.is_all_day() {
            LocalizedTimeFormat::WeekdayDate
        } else {
            LocalizedTimeFormat::WeekdayDateAndMinute
        },
    }
}

pub(super) fn calendar_count_caption(agenda: &CaldavAgenda) -> String {
    format!("{} calendars over CalDAV.", agenda.calendar_count)
}

pub(super) fn nextcloud_footer(
    app_label: &str,
    provider: &CaldavProvider,
    app_url: fn(&NextcloudWebRoot) -> String,
) -> Section {
    Section::dictionary(provider.nextcloud_web_root().map(|nextcloud| {
        KeyValue::new(
            "Nextcloud",
            Cell::Link {
                label: app_label.to_owned(),
                url: app_url(nextcloud),
                glyph: Some(SimpleIcon::Nextcloud.into()),
            },
        )
    }))
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

impl BarWidget for CaldavAgenda {
    fn name(&self) -> &'static str {
        "Upcoming events"
    }

    fn icon(&self) -> Glyph {
        Phosphor::CalendarDots(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        let now = clock.local();
        let event = self.next_event(now)?;
        let summary: String = event.summary.chars().take(LABEL_CHARACTERS).collect();
        Some(format!(
            "{}: {summary}",
            starts_label(event.start, now, locale)
        ))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        let now = clock.local();
        if self.events.iter().any(|event| event.is_ongoing(now)) {
            WidgetState::On
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let now = clock.local();
        let rows = self
            .events
            .iter()
            .filter(|event| !event.has_ended(now))
            .map(|event| EventRow {
                event,
                provider: &self.provider,
            });
        TooltipBody::sections([
            Section::truncated_column_table(
                [
                    ColumnHeading::titled("Date"),
                    ColumnHeading::titled("Time"),
                    ColumnHeading::titled("Calendar").align_x(Horizontal::Left),
                    ColumnHeading::titled("Location").align_x(Horizontal::Left),
                ],
                rows,
                LISTED_EVENTS,
            )
            .titled(("Upcoming events", calendar_count_caption(self))),
            nextcloud_footer(
                "open Calendar",
                &self.provider,
                NextcloudWebRoot::calendar_app,
            ),
        ])
    }
}
