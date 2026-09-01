use std::time::Duration;

use crate::state::{StoreError, open};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS idle_timeouts (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    lock_user INTEGER,
    screen_off INTEGER,
    standby INTEGER,
    power_down INTEGER
)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timeouts {
    pub lock_user: Option<Duration>,
    pub screen_off: Option<Duration>,
    pub standby: Option<Duration>,
    pub power_down: Option<Duration>,
}

pub async fn load() -> Timeouts {
    match try_load().await {
        Ok(timeouts) => timeouts,
        Err(e) => {
            tracing::warn!("idle timeouts: load failed: {e}");
            Timeouts::default()
        }
    }
}

pub async fn save(timeouts: Timeouts) -> bool {
    match try_save(timeouts).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("idle timeouts: save failed: {e}");
            false
        }
    }
}

fn secs(timeout: Option<Duration>) -> Option<i64> {
    timeout.map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn duration(row: &turso::Row, index: usize) -> Option<Duration> {
    let value = row.get_value(index).ok()?;
    let secs = value.as_integer().copied()?;
    Some(Duration::from_secs(u64::try_from(secs).unwrap_or(0)))
}

async fn try_load() -> Result<Timeouts, StoreError> {
    let conn = open().await?;
    let mut rows = conn
        .query(
            "SELECT lock_user, screen_off, standby, power_down FROM idle_timeouts WHERE id = 0",
            (),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(Timeouts::default());
    };
    Ok(Timeouts {
        lock_user: duration(&row, 0),
        screen_off: duration(&row, 1),
        standby: duration(&row, 2),
        power_down: duration(&row, 3),
    })
}

async fn try_save(timeouts: Timeouts) -> Result<(), StoreError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO idle_timeouts (id, lock_user, screen_off, standby, power_down)
         VALUES (0, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET lock_user = ?1, screen_off = ?2, standby = ?3, power_down = \
         ?4",
        (
            secs(timeouts.lock_user),
            secs(timeouts.screen_off),
            secs(timeouts.standby),
            secs(timeouts.power_down),
        ),
    )
    .await?;
    Ok(())
}
