use std::convert::Infallible;

use chrono::{Weekday, WeekdaySet};
use futures::{Stream, StreamExt, stream};
use sottomano_sources::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    sqlite_state_store::{Columns, StoreError, with_table},
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS working_days (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    weekday_bits INTEGER NOT NULL
)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkingDays {
    pub weekdays: WeekdaySet,
}

impl Default for WorkingDays {
    fn default() -> Self {
        Self {
            weekdays: WeekdaySet::from_array([
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ]),
        }
    }
}

impl WorkingDays {
    #[must_use]
    pub fn toggled(self, day: Weekday) -> Self {
        let mut weekdays = self.weekdays;
        if !weekdays.remove(day) {
            weekdays.insert(day);
        }
        Self { weekdays }
    }

    fn bits(self) -> i64 {
        self.weekdays
            .iter(Weekday::Mon)
            .map(|day| 1 << day.num_days_from_monday())
            .sum()
    }

    fn of_bits(bits: i64) -> Self {
        Self {
            weekdays: WeekdaySet::ALL
                .iter(Weekday::Mon)
                .filter(|day| bits & (1 << day.num_days_from_monday()) != 0)
                .collect(),
        }
    }
}

impl ReadingSource for WorkingDays {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "working days";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(load()).map(Ok).map(SourceDelivery::Reading)
    }
}

async fn load() -> WorkingDays {
    with_table("working days: load failed", &[CREATE_TABLE], read_days)
        .await
        .unwrap_or_default()
}

pub async fn save(days: WorkingDays) -> bool {
    with_table("working days: save failed", &[CREATE_TABLE], async |conn| {
        write_days(conn, days).await
    })
    .await
    .is_some()
}

async fn read_days(conn: &turso::Connection) -> Result<WorkingDays, StoreError> {
    let mut rows = conn
        .query("SELECT weekday_bits FROM working_days WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(WorkingDays::default());
    };
    Ok(WorkingDays::of_bits(columns.integer(&row, "weekday_bits")?))
}

async fn write_days(conn: &turso::Connection, days: WorkingDays) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO working_days (id, weekday_bits) VALUES (0, ?1)
         ON CONFLICT(id) DO UPDATE SET weekday_bits = ?1",
        (days.bits(),),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Weekday, WeekdaySet};
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, WorkingDays, read_days, write_days};

    #[tokio::test]
    async fn an_unsaved_week_reads_as_monday_to_friday() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;

        assert_eq!(read_days(&conn).await.unwrap(), WorkingDays::default());
    }

    #[tokio::test]
    async fn chosen_days_survive_a_round_trip() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        let days = WorkingDays {
            weekdays: WeekdaySet::from_array([Weekday::Mon, Weekday::Tue, Weekday::Thu]),
        };
        write_days(&conn, days).await.unwrap();

        assert_eq!(read_days(&conn).await.unwrap(), days);
    }

    #[tokio::test]
    async fn an_empty_week_round_trips() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        let days = WorkingDays {
            weekdays: WeekdaySet::EMPTY,
        };
        write_days(&conn, days).await.unwrap();

        assert_eq!(read_days(&conn).await.unwrap(), days);
    }

    #[test]
    fn toggling_a_day_twice_restores_the_week() {
        let week = WorkingDays::default();

        assert!(!week.toggled(Weekday::Wed).weekdays.contains(Weekday::Wed));
        assert!(week.toggled(Weekday::Sat).weekdays.contains(Weekday::Sat));
        assert_eq!(week.toggled(Weekday::Wed).toggled(Weekday::Wed), week);
    }
}
