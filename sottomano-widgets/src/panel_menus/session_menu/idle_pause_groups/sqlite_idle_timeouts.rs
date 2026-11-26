use std::{collections::HashMap, time::Duration};

use sottomano_desktop_sources::{
    login_session_presence::idle_session_actions::IdleAction,
    sqlite_state_store::{Columns, StoreError, in_state_store, with_state_store},
};

use crate::widgets::idle::IdleTimeout;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Timeouts {
    enabled_timeouts: HashMap<IdleAction, Duration>,
}

impl FromIterator<(IdleAction, Duration)> for Timeouts {
    fn from_iter<Enabled: IntoIterator<Item = (IdleAction, Duration)>>(enabled: Enabled) -> Self {
        Self {
            enabled_timeouts: enabled.into_iter().collect(),
        }
    }
}

impl Timeouts {
    #[must_use]
    pub fn timeout_for(&self, action: IdleAction) -> IdleTimeout {
        match self.enabled_timeouts.get(&action) {
            Some(timeout) => IdleTimeout::After(*timeout),
            None => IdleTimeout::Never,
        }
    }
}

fn idle_action_store_key(action: IdleAction) -> &'static str {
    match action {
        IdleAction::Lock => "lock_user",
        IdleAction::ScreenOff => "screen_off",
        IdleAction::Sleep => "sleep",
        IdleAction::Hibernate => "hibernate",
        IdleAction::PowerOff => "power_down",
    }
}

pub async fn load() -> Timeouts {
    with_state_store("idle timeouts: load failed", read_timeouts)
        .await
        .unwrap_or_default()
}

pub async fn save(timeouts: Timeouts) -> Result<(), StoreError> {
    in_state_store(async |connection| write_timeouts(connection, &timeouts).await).await
}

async fn read_timeouts(connection: &turso::Connection) -> Result<Timeouts, StoreError> {
    let mut rows = connection
        .query(
            "SELECT idle_action, after_seconds FROM idle_action_offsets",
            (),
        )
        .await?;
    let columns = Columns::from(&rows);
    let mut enabled_timeouts = HashMap::new();
    while let Some(row) = rows.next().await? {
        let after = Duration::from_secs(columns.ranged_integer(&row, "after_seconds")?);
        let stored_key = columns.text(&row, "idle_action")?;
        if let Some(action) = IdleAction::ALL
            .into_iter()
            .find(|action| idle_action_store_key(*action) == stored_key)
        {
            enabled_timeouts.insert(action, after);
        }
    }
    Ok(Timeouts { enabled_timeouts })
}

async fn write_timeouts(
    connection: &turso::Connection,
    timeouts: &Timeouts,
) -> Result<(), StoreError> {
    connection
        .execute("DELETE FROM idle_action_offsets", ())
        .await?;
    for (action, duration) in &timeouts.enabled_timeouts {
        connection
            .execute(
                "INSERT INTO idle_action_offsets (idle_action, after_seconds) VALUES (?1, ?2)",
                (
                    idle_action_store_key(*action),
                    i64::try_from(duration.as_secs())
                        .expect("an idle offset is shorter than i64::MAX seconds"),
                ),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_desktop_sources::{
        login_session_presence::idle_session_actions::IdleAction,
        sqlite_state_store::migrated_in_memory_store,
    };

    use super::{Timeouts, read_timeouts, write_timeouts};

    #[tokio::test]
    async fn timeouts_survive_a_round_trip() {
        let connection = migrated_in_memory_store().await;
        let timeouts = Timeouts::from_iter([
            (IdleAction::Lock, Duration::from_secs(60)),
            (IdleAction::Sleep, Duration::from_secs(600)),
            (IdleAction::Hibernate, Duration::from_mins(30)),
            (IdleAction::PowerOff, Duration::from_secs(3600)),
        ]);
        write_timeouts(&connection, &timeouts).await.unwrap();

        assert_eq!(read_timeouts(&connection).await.unwrap(), timeouts);
    }

    #[tokio::test]
    async fn a_fresh_store_locks_and_blanks_together_then_sleeps_but_never_hibernates() {
        let connection = migrated_in_memory_store().await;

        assert_eq!(
            read_timeouts(&connection).await.unwrap(),
            Timeouts::from_iter([
                (IdleAction::Lock, Duration::from_mins(5)),
                (IdleAction::ScreenOff, Duration::ZERO),
                (IdleAction::Sleep, Duration::from_mins(15)),
            ])
        );
    }

    #[tokio::test]
    async fn disabling_every_timeout_survives_a_reload() {
        let connection = migrated_in_memory_store().await;
        write_timeouts(&connection, &Timeouts::default())
            .await
            .unwrap();

        assert_eq!(
            read_timeouts(&connection).await.unwrap(),
            Timeouts::default()
        );
    }
}
