use std::time::Duration;

use chrono::{NaiveTime, TimeDelta, Timelike};
use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, flag, in_state_store, with_state_store,
};

use crate::widget_groups::quiet_schedule_group::quiet_window::QuietScheduleAdherence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietHoursSchedule {
    pub schedule_adherence: QuietScheduleAdherence,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub screen_off: Option<Duration>,
}

impl Default for QuietHoursSchedule {
    fn default() -> Self {
        Self {
            schedule_adherence: QuietScheduleAdherence::Followed,
            start: NaiveTime::MIN + TimeDelta::hours(23),
            end: NaiveTime::MIN + TimeDelta::hours(7),
            screen_off: Some(Duration::from_mins(1)),
        }
    }
}

pub async fn load() -> QuietHoursSchedule {
    with_state_store("quiet hours schedule: load failed", read_schedule)
        .await
        .unwrap_or_default()
}

pub async fn save(schedule: QuietHoursSchedule) -> Result<(), StoreError> {
    in_state_store(async |connection| write_schedule(connection, schedule).await).await
}

fn minutes_since_midnight(time: NaiveTime) -> i64 {
    i64::from(time.hour() * 60 + time.minute())
}

fn time_of_minutes(minutes: i64) -> NaiveTime {
    NaiveTime::MIN + TimeDelta::minutes(minutes)
}

async fn read_schedule(connection: &turso::Connection) -> Result<QuietHoursSchedule, StoreError> {
    let mut rows = connection
        .query(
            "SELECT follows_schedule, start_minutes, end_minutes FROM quiet_hours WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(QuietHoursSchedule::default());
    };
    Ok(QuietHoursSchedule {
        schedule_adherence: match columns.integer(&row, "follows_schedule")? {
            0 => QuietScheduleAdherence::Ignored,
            _ => QuietScheduleAdherence::Followed,
        },
        start: time_of_minutes(columns.integer(&row, "start_minutes")?),
        end: time_of_minutes(columns.integer(&row, "end_minutes")?),
        screen_off: read_screen_off(connection).await?,
    })
}

async fn read_screen_off(connection: &turso::Connection) -> Result<Option<Duration>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT after_seconds FROM quiet_hours_screen_off WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(Some(Duration::from_secs(
        columns.ranged_integer(&row, "after_seconds")?,
    )))
}

async fn write_schedule(
    connection: &turso::Connection,
    schedule: QuietHoursSchedule,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO quiet_hours (id, follows_schedule, start_minutes, end_minutes)
         VALUES (0, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET follows_schedule = ?1, start_minutes = ?2, end_minutes = ?3",
            (
                flag(schedule.schedule_adherence == QuietScheduleAdherence::Followed),
                minutes_since_midnight(schedule.start),
                minutes_since_midnight(schedule.end),
            ),
        )
        .await?;
    connection
        .execute("DELETE FROM quiet_hours_screen_off", ())
        .await?;
    if let Some(timeout) = schedule.screen_off {
        connection
            .execute(
                "INSERT INTO quiet_hours_screen_off (id, after_seconds) VALUES (0, ?1)",
                (i64::try_from(timeout.as_secs())
                    .expect("a screen-off timeout is shorter than i64::MAX seconds"),),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::NaiveTime;
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{QuietHoursSchedule, read_schedule, write_schedule};
    use crate::widget_groups::quiet_schedule_group::quiet_window::QuietScheduleAdherence;

    #[tokio::test]
    async fn schedule_survives_a_round_trip() {
        let connection = migrated_in_memory_store().await;
        let schedule = QuietHoursSchedule {
            schedule_adherence: QuietScheduleAdherence::Ignored,
            start: NaiveTime::from_hms_opt(22, 15, 0).unwrap(),
            end: NaiveTime::from_hms_opt(6, 45, 0).unwrap(),
            screen_off: Some(Duration::from_secs(30)),
        };
        write_schedule(&connection, schedule).await.unwrap();

        assert_eq!(read_schedule(&connection).await.unwrap(), schedule);
    }

    #[tokio::test]
    async fn disabled_screen_off_round_trips_as_null() {
        let connection = migrated_in_memory_store().await;
        let schedule = QuietHoursSchedule {
            screen_off: None,
            ..QuietHoursSchedule::default()
        };
        write_schedule(&connection, schedule).await.unwrap();

        assert_eq!(read_schedule(&connection).await.unwrap(), schedule);
    }
}
