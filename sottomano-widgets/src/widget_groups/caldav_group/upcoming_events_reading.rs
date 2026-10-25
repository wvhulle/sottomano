use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_sources::caldav_tasks_events::{
    agenda_moment_zones::AgendaMoment,
    caldav_agenda::{CaldavAgenda, ChangeDelivery},
    caldav_event_query::UpcomingEvent,
    nextcloud_web_links::NextcloudWebRoot,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_durations::{relative_day_word, relative_moment_text},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, Section, TableRecord, TableRow, TooltipBody,
        },
        widget_state::WidgetState,
    },
};

const LISTED_EVENTS: usize = 10;
const LABEL_CHARACTERS: usize = 28;

struct EventRow<'a> {
    event: &'a UpcomingEvent,
    nextcloud: Option<&'a NextcloudWebRoot>,
}

impl TableRow<3> for EventRow<'_> {
    fn table_record(self) -> TableRecord<3> {
        TableRecord::new(
            linked_title(
                &self.event.summary,
                self.nextcloud
                    .map(|nextcloud| nextcloud.event_details(self.event)),
            ),
            [
                Some(moment_cell(self.event.start)),
                Some(Cell::Hint(self.event.calendar.clone())),
                self.event.location.clone().map(Cell::from),
            ],
        )
    }
}

pub(super) fn linked_title(title: &str, details_url: Option<String>) -> Cell {
    match details_url {
        Some(url) => Cell::Link {
            label: title.to_owned(),
            url,
        },
        None => title.into(),
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

pub(super) fn change_delivery_caption(agenda: &CaldavAgenda) -> String {
    let checked = match agenda.change_delivery {
        ChangeDelivery::NextcloudNotifyPush => {
            "refreshed when Nextcloud notify_push reports activity"
        }
        ChangeDelivery::ChangeTokenPolling => "calendar change tokens checked every minute",
    };
    format!(
        "{} calendars over CalDAV, {checked}.",
        agenda.calendar_count
    )
}

pub(super) fn nextcloud_footer(app_label: &str, app_url: Option<String>, hidden: usize) -> Section {
    Section::dictionary(
        [
            (hidden > 0).then(|| KeyValue::new("Not shown", format!("{hidden} more"))),
            app_url.map(|url| {
                KeyValue::new(
                    "Nextcloud",
                    Cell::Link {
                        label: app_label.to_owned(),
                        url,
                    },
                )
            }),
        ]
        .into_iter()
        .flatten(),
    )
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
        let upcoming: Vec<&UpcomingEvent> = self
            .events
            .iter()
            .filter(|event| !event.has_ended(now))
            .collect();
        let hidden = upcoming.len().saturating_sub(LISTED_EVENTS);
        let rows = upcoming
            .into_iter()
            .take(LISTED_EVENTS)
            .map(|event| EventRow {
                event,
                nextcloud: self.provider.nextcloud_web_root(),
            });
        TooltipBody::sections([
            Section::column_table(
                [
                    ColumnHeading::new("Starts"),
                    ColumnHeading::new("Calendar").align_x(Horizontal::Left),
                    ColumnHeading::new("Location").align_x(Horizontal::Left),
                ],
                rows,
            )
            .titled(("Upcoming events", change_delivery_caption(self))),
            nextcloud_footer(
                "open Calendar",
                self.provider
                    .nextcloud_web_root()
                    .map(NextcloudWebRoot::calendar_app),
                hidden,
            ),
        ])
    }
}
