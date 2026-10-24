use chrono::{DateTime, Local, Locale};
use iced::alignment::Horizontal;
use sottomano_sources::caldav_tasks_events::{
    caldav_agenda::{AgendaMoment, CaldavAgenda, ChangeDelivery},
    caldav_event_query::UpcomingEvent,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

const LISTED_EVENTS: usize = 12;
const LABEL_CHARACTERS: usize = 28;

impl TableRow<3> for &UpcomingEvent {
    fn table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.summary.as_str(),
            [
                Some(moment_cell(self.start)),
                Some(Cell::Hint(self.calendar.clone())),
                self.location.clone().map(Cell::from),
            ],
        )
    }
}

pub(super) fn moment_cell(moment: AgendaMoment) -> Cell {
    Cell::Time {
        at: moment.starts_at().to_utc(),
        format: match moment {
            AgendaMoment::AllDay(_) => LocalizedTimeFormat::Date,
            AgendaMoment::Timed(_) => LocalizedTimeFormat::DateAndTime,
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

fn starts_label(start: AgendaMoment, now: DateTime<Local>, locale: Locale) -> String {
    let today = now.date_naive();
    match start {
        _ if start.starts_at() <= now => "now".to_owned(),
        AgendaMoment::AllDay(day) if day == today => "today".to_owned(),
        AgendaMoment::AllDay(_) => LocalizedTimeFormat::Weekday.format(&start.starts_at(), locale),
        AgendaMoment::Timed(at) if at.date_naive() == today => {
            LocalizedTimeFormat::HourAndMinute.format(&at, locale)
        }
        AgendaMoment::Timed(at) => format!(
            "{} {}",
            LocalizedTimeFormat::Weekday.format(&at, locale),
            LocalizedTimeFormat::HourAndMinute.format(&at, locale)
        ),
    }
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
            "{} {summary}",
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
        let upcoming = self
            .events
            .iter()
            .filter(|event| !event.has_ended(now))
            .take(LISTED_EVENTS);
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Starts"),
                ColumnHeading::new("Calendar").align_x(Horizontal::Left),
                ColumnHeading::new("Location").align_x(Horizontal::Left),
            ],
            upcoming,
        )
        .titled(("Upcoming events", change_delivery_caption(self)))])
    }
}
