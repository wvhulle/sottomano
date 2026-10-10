use std::time::Duration;

use crate::state::{Columns, StoreError, connect};

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

async fn try_load() -> Result<Timeouts, StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
    read_timeouts(&conn).await
}

async fn read_timeouts(conn: &turso::Connection) -> Result<Timeouts, StoreError> {
    let mut rows = conn
        .query(
            "SELECT lock_user, screen_off, standby, power_down FROM idle_timeouts WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(Timeouts::default());
    };
    Ok(Timeouts {
        lock_user: columns.duration(&row, "lock_user"),
        screen_off: columns.duration(&row, "screen_off"),
        standby: columns.duration(&row, "standby"),
        power_down: columns.duration(&row, "power_down"),
    })
}

async fn try_save(timeouts: Timeouts) -> Result<(), StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
    write_timeouts(&conn, timeouts).await
}

async fn write_timeouts(conn: &turso::Connection, timeouts: Timeouts) -> Result<(), StoreError> {
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{CREATE_TABLE, Timeouts, read_timeouts, write_timeouts};

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn timeouts_survive_a_round_trip() {
        let conn = connection().await;
        let timeouts = Timeouts {
            lock_user: Some(Duration::from_secs(60)),
            screen_off: None,
            standby: Some(Duration::from_secs(600)),
            power_down: Some(Duration::from_secs(3600)),
        };
        write_timeouts(&conn, timeouts).await.unwrap();

        assert_eq!(read_timeouts(&conn).await.unwrap(), timeouts);
    }
}
