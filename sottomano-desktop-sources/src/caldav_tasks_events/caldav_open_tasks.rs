use chrono::{DateTime, Local};
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    caldav_tasks_events::{
        caldav_agenda::{
            AgendaSnapshotPolling, CaldavAccount, CalendarEntryQuery, CalendarHolding,
            CalendarHomePoller, Error, push_or_poll_deliveries,
        },
        caldav_calendar_discovery::{CalendarCollection, CalendarHome},
        caldav_event_query::EventWindow,
        caldav_server_provider::CaldavProvider,
        caldav_task_query::{OpenTask, TaskDueStatus, query_open_tasks},
    },
    system_protocol_clients::dbus_desktop_buses::OnBus,
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
            .filter(|task| task.due_status(now) == TaskDueStatus::Overdue)
            .count()
    }
}

impl CalendarEntryQuery for OpenTask {
    fn holding(calendar: &CalendarCollection) -> CalendarHolding {
        if calendar.holds_tasks {
            CalendarHolding::Holds
        } else {
            CalendarHolding::Lacks
        }
    }

    async fn queried(
        home: &CalendarHome,
        calendar: &CalendarCollection,
        _window: EventWindow,
    ) -> Result<Vec<Self>, Error> {
        query_open_tasks(home, calendar).await
    }
}

struct OpenTaskPoller {
    home_poller: CalendarHomePoller<OpenTask>,
}

impl AgendaSnapshotPolling for OpenTaskPoller {
    type Snapshot = OpenTaskList;

    async fn polled_snapshot(&mut self) -> Result<OpenTaskList, Error> {
        let polled_home = self.home_poller.poll().await?;
        let mut tasks: Vec<OpenTask> = self.home_poller.fetched_entries().cloned().collect();
        tasks.sort_by(OpenTask::earliest_due_first);
        Ok(OpenTaskList {
            tasks,
            task_list_count: self.home_poller.fetched_calendar_count(),
            provider: polled_home.provider,
        })
    }
}

impl SnapshotSource for OpenTaskList {
    type Error = Error;
    type StreamInputs = OnBus<CaldavAccount>;

    const NAME: &'static str = "Open tasks";

    fn delivery_stream(
        account: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        push_or_poll_deliveries(account, |account| OpenTaskPoller {
            home_poller: CalendarHomePoller::new(account),
        })
    }
}
