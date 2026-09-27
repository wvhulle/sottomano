use chrono::TimeDelta;

use crate::{
    data::sun_schedule::SunEventOffsets,
    state::{Columns, StoreError, open},
};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS dark_mode_schedule (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    armed INTEGER NOT NULL,
    sunrise_offset INTEGER NOT NULL,
    sunset_offset INTEGER NOT NULL
)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkModeSchedule {
    pub auto_switch_armed: bool,
    pub sun_event_offsets: SunEventOffsets,
}

impl Default for DarkModeSchedule {
    fn default() -> Self {
        Self {
            auto_switch_armed: true,
            sun_event_offsets: SunEventOffsets::default(),
        }
    }
}

pub async fn load() -> DarkModeSchedule {
    match try_load().await {
        Ok(schedule) => schedule,
        Err(e) => {
            tracing::warn!("dark mode schedule: load failed: {e}");
            DarkModeSchedule::default()
        }
    }
}

pub async fn save(schedule: DarkModeSchedule) -> bool {
    match try_save(schedule).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("dark mode schedule: save failed: {e}");
            false
        }
    }
}

async fn try_load() -> Result<DarkModeSchedule, StoreError> {
    let conn = open().await?;
    read_schedule(&conn).await
}

async fn read_schedule(conn: &turso::Connection) -> Result<DarkModeSchedule, StoreError> {
    let mut rows = conn
        .query(
            "SELECT armed, sunrise_offset, sunset_offset FROM dark_mode_schedule WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(DarkModeSchedule::default());
    };
    Ok(DarkModeSchedule {
        auto_switch_armed: columns.integer(&row, "armed")? != 0,
        sun_event_offsets: SunEventOffsets {
            sunrise_offset: TimeDelta::seconds(columns.integer(&row, "sunrise_offset")?),
            sunset_offset: TimeDelta::seconds(columns.integer(&row, "sunset_offset")?),
        },
    })
}

async fn try_save(schedule: DarkModeSchedule) -> Result<(), StoreError> {
    let conn = open().await?;
    write_schedule(&conn, schedule).await
}

async fn write_schedule(
    conn: &turso::Connection,
    schedule: DarkModeSchedule,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO dark_mode_schedule (id, armed, sunrise_offset, sunset_offset)
         VALUES (0, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET armed = ?1, sunrise_offset = ?2, sunset_offset = ?3",
        (
            i64::from(schedule.auto_switch_armed),
            schedule.sun_event_offsets.sunrise_offset.num_seconds(),
            schedule.sun_event_offsets.sunset_offset.num_seconds(),
        ),
    )
    .await?;
    Ok(())
}
