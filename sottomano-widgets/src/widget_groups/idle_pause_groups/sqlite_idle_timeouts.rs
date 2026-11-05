use std::time::Duration;

use sottomano_desktop_sources::{
    login_session_presence::{
        idle_session_actions::IdleAction, session_power_actions::SessionAction,
    },
    sqlite_state_store::{Columns, StoreError, with_table},
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS idle_action_timeouts (
    idle_action TEXT PRIMARY KEY,
    after_seconds INTEGER NOT NULL
)";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Timeouts {
    pub lock_user: Option<Duration>,
    pub screen_off: Option<Duration>,
    pub sleep: Option<Duration>,
    pub hibernate: Option<Duration>,
    pub power_down: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledAfterIdle {
    Never,
    Disabled,
    After(Duration),
}

impl Timeouts {
    #[must_use]
    pub fn timeout_for(self, action: IdleAction) -> Option<Duration> {
        match action {
            IdleAction::Lock => self.lock_user,
            IdleAction::ScreenOff => self.screen_off,
            IdleAction::Sleep => self.sleep,
            IdleAction::Hibernate => self.hibernate,
            IdleAction::PowerOff => self.power_down,
        }
    }

    #[must_use]
    pub fn scheduled(self, action: SessionAction) -> ScheduledAfterIdle {
        IdleAction::performing(action).map_or(ScheduledAfterIdle::Never, |idle| {
            self.timeout_for(idle)
                .map_or(ScheduledAfterIdle::Disabled, ScheduledAfterIdle::After)
        })
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
        async |connection| write_timeouts(connection, timeouts).await,
    )
    .await
    .is_some()
}

async fn read_timeouts(connection: &turso::Connection) -> Result<Timeouts, StoreError> {
    let mut rows = connection
        .query(
            "SELECT idle_action, after_seconds FROM idle_action_timeouts",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut timeouts = Timeouts::default();
    while let Some(row) = rows.next().await? {
        let after = Some(Duration::from_secs(
            u64::try_from(columns.integer(&row, "after_seconds")?).unwrap_or_default(),
        ));
        match columns.text(&row, "idle_action")?.as_str() {
            "lock_user" => timeouts.lock_user = after,
            "screen_off" => timeouts.screen_off = after,
            "sleep" => timeouts.sleep = after,
            "hibernate" => timeouts.hibernate = after,
            "power_down" => timeouts.power_down = after,
            _ => {}
        }
    }
    Ok(timeouts)
}

async fn write_timeouts(
    connection: &turso::Connection,
    timeouts: Timeouts,
) -> Result<(), StoreError> {
    connection
        .execute("DELETE FROM idle_action_timeouts", ())
        .await?;
    let enabled_actions = [
        ("lock_user", timeouts.lock_user),
        ("screen_off", timeouts.screen_off),
        ("sleep", timeouts.sleep),
        ("hibernate", timeouts.hibernate),
        ("power_down", timeouts.power_down),
    ];
    for (idle_action, timeout) in enabled_actions {
        if let Some(duration) = timeout {
            connection
                .execute(
                    "INSERT INTO idle_action_timeouts (idle_action, after_seconds) VALUES (?1, ?2)",
                    (
                        idle_action,
                        i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::{
        login_session_presence::session_power_actions::SessionAction,
        sqlite_state_store::in_memory_connection,
    };

    use super::{CREATE_TABLE, ScheduledAfterIdle, Timeouts, read_timeouts, write_timeouts};

    #[tokio::test]
    async fn timeouts_survive_a_round_trip() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let timeouts = Timeouts {
            lock_user: Some(Duration::from_secs(60)),
            screen_off: None,
            sleep: Some(Duration::from_secs(600)),
            hibernate: Some(Duration::from_mins(30)),
            power_down: Some(Duration::from_secs(3600)),
        };
        write_timeouts(&connection, timeouts).await.unwrap();

        assert_eq!(read_timeouts(&connection).await.unwrap(), timeouts);
    }

    #[test]
    fn the_hibernate_timeout_schedules_logind_hibernate() {
        let timeouts = Timeouts {
            hibernate: Some(Duration::from_mins(30)),
            ..Timeouts::default()
        };

        assert_eq!(
            timeouts.scheduled(SessionAction::Hibernate),
            ScheduledAfterIdle::After(Duration::from_mins(30))
        );
    }

    #[test]
    fn the_sleep_timeout_schedules_logind_sleep_and_never_a_plain_suspend() {
        let timeouts = Timeouts {
            sleep: Some(Duration::from_secs(600)),
            ..Timeouts::default()
        };

        assert_eq!(
            timeouts.scheduled(SessionAction::Sleep),
            ScheduledAfterIdle::After(Duration::from_secs(600))
        );
        assert_eq!(
            timeouts.scheduled(SessionAction::Suspend),
            ScheduledAfterIdle::Never
        );
    }
}
