use std::time::Duration;

use crate::{
    data::source::session_action::SessionAction,
    state::{Columns, StoreError, with_table},
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS idle_timeouts (
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledAfterIdle {
    Never,
    Disabled,
    After(Duration),
}

impl Timeouts {
    pub fn scheduled(self, action: SessionAction) -> ScheduledAfterIdle {
        let timeout = match action {
            SessionAction::Lock => self.lock_user,
            SessionAction::ScreenOff => self.screen_off,
            SessionAction::Suspend => self.standby,
            SessionAction::PowerOff => self.power_down,
            _ => return ScheduledAfterIdle::Never,
        };
        timeout.map_or(ScheduledAfterIdle::Disabled, ScheduledAfterIdle::After)
    }
}

pub async fn load() -> Timeouts {
    with_table("idle timeouts: load failed", &[CREATE_TABLE], read_timeouts)
        .await
        .unwrap_or_default()
}

pub async fn save(timeouts: Timeouts) -> bool {
    with_table(
        "idle timeouts: save failed",
        &[CREATE_TABLE],
        async |conn| write_timeouts(conn, timeouts).await,
    )
    .await
    .is_some()
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

async fn write_timeouts(conn: &turso::Connection, timeouts: Timeouts) -> Result<(), StoreError> {
    let seconds = |timeout: Option<Duration>| {
        timeout.map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
    };
    conn.execute(
        "INSERT INTO idle_timeouts (id, lock_user, screen_off, standby, power_down)
         VALUES (0, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET lock_user = ?1, screen_off = ?2, standby = ?3, power_down = \
         ?4",
        (
            seconds(timeouts.lock_user),
            seconds(timeouts.screen_off),
            seconds(timeouts.standby),
            seconds(timeouts.power_down),
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
