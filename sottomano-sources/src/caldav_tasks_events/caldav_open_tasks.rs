use chrono::{DateTime, Local};
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    caldav_tasks_events::{
        caldav_agenda::{
            AgendaSnapshotPolling, CaldavAccount, CalendarEntryQuery, CalendarHomePoller, Error,
            push_or_poll_deliveries,
        },
        caldav_calendar_discovery::{CalendarCollection, CalendarHome},
        caldav_event_query::EventWindow,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, query_open_tasks},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTaskList {
    pub tasks: Vec<OpenTask>,
    pub task_list_count: usize,
    pub provider: CaldavProvider,
}

impl OpenTaskList {
    #[must_use]
    pub fn overdue_task_count(&self, now: DateTime<Local>) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.is_overdue(now))
            .count()
    }
}

impl CalendarEntryQuery for OpenTask {
    fn held_by(calendar: &CalendarCollection) -> bool {
        calendar.holds_tasks
    }

    async fn queried(
        home: &CalendarHome,
        calendar: &CalendarCollection,
        _window: EventWindow,
    ) -> Result<Vec<Self>, Error> {
        query_open_tasks(home, calendar).await
    }
}

struct OpenTaskPoller(CalendarHomePoller<OpenTask>);

impl AgendaSnapshotPolling for OpenTaskPoller {
    type Snapshot = OpenTaskList;

    async fn polled_snapshot(&mut self) -> Result<OpenTaskList, Error> {
        let polled_home = self.0.poll().await?;
        let mut tasks: Vec<OpenTask> = self.0.fetched_entries().cloned().collect();
        tasks.sort_by(OpenTask::earliest_due_first);
        Ok(OpenTaskList {
            tasks,
            task_list_count: self.0.fetched_calendar_count(),
            provider: polled_home.provider,
        })
    }
}

impl SnapshotSource for OpenTaskList {
    type Error = Error;
    type StreamInputs = CaldavAccount;

    const NAME: &'static str = "CalDAV tasks";

    fn delivery_stream(
        account: CaldavAccount,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        push_or_poll_deliveries(
            &account,
            OpenTaskPoller(CalendarHomePoller::new(account.clone())),
        )
    }
}
