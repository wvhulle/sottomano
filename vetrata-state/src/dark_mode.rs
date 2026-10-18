use std::convert::Infallible;

use chrono::TimeDelta;
use futures::{Stream, StreamExt, stream};
use vetrata_sources::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    store::{Columns, StoreError, with_table},
    sun_schedule::SunEventOffsets,
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS dark_mode_schedule (
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

impl ReadingSource for DarkModeSchedule {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "dark mode schedule";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(load()).map(Ok).map(SourceDelivery::Reading)
    }
}

async fn load() -> DarkModeSchedule {
    with_table(
        "dark mode schedule: load failed",
        &[CREATE_TABLE],
        read_schedule,
    )
    .await
    .unwrap_or_default()
}

pub async fn save(schedule: DarkModeSchedule) -> bool {
    with_table(
        "dark mode schedule: save failed",
        &[CREATE_TABLE],
        async |conn| write_schedule(conn, schedule).await,
    )
    .await
    .is_some()
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
