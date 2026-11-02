use chrono::{DateTime, Duration, Utc};
use sottomano_sources::{
    desktop_windows_apps::app_launch_history::Launch,
    sqlite_state_store::{Columns, StoreError, with_table},
};

const TTL_DAYS: i64 = 60;

pub const SCHEMA: [&str; 1] = ["CREATE TABLE IF NOT EXISTS launch_events (
    workspace TEXT NOT NULL,
    app_id TEXT NOT NULL,
    launched_at TEXT NOT NULL,
    PRIMARY KEY (workspace, app_id, launched_at)
)"];

pub async fn load() -> Vec<Launch> {
    with_table("launch store: load failed", &SCHEMA, read_launches)
        .await
        .unwrap_or_default()
}

pub async fn record(launch: Launch) {
    with_table("launch store: record failed", &SCHEMA, async |connection| {
        write_launch(connection, &launch).await
    })
    .await;
}

async fn read_launches(connection: &turso::Connection) -> Result<Vec<Launch>, StoreError> {
    let cutoff = (Utc::now() - Duration::days(TTL_DAYS)).to_rfc3339();
    connection
        .execute(
            "DELETE FROM launch_events WHERE launched_at < ?1",
            (cutoff,),
        )
        .await?;
    let mut rows = connection
        .query(
            "SELECT workspace, app_id, launched_at FROM launch_events",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut launches = Vec::new();
    while let Some(row) = rows.next().await? {
        let Ok(launched_at) = DateTime::parse_from_rfc3339(&columns.text(&row, "launched_at")?)
        else {
            continue;
        };
        launches.push(Launch {
            workspace: columns.text(&row, "workspace")?,
            app_id: columns.text(&row, "app_id")?,
            launched_at: launched_at.with_timezone(&Utc),
        });
    }
    Ok(launches)
}

async fn write_launch(connection: &turso::Connection, launch: &Launch) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO launch_events (workspace, app_id, launched_at) VALUES (?1, ?2, ?3) ON \
             CONFLICT DO NOTHING",
            (
                launch.workspace.clone(),
                launch.app_id.clone(),
                launch.launched_at.to_rfc3339(),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{Launch, SCHEMA, read_launches, write_launch};

    fn firefox_on_web(minutes_ago: i64) -> Launch {
        Launch {
            workspace: "web".to_owned(),
            app_id: "firefox".to_owned(),
            launched_at: Utc::now() - Duration::minutes(minutes_ago),
        }
    }

    #[tokio::test]
    async fn a_launch_survives_a_round_trip() {
        let connection = in_memory_connection(&SCHEMA).await;
        let launch = firefox_on_web(0);
        write_launch(&connection, &launch).await.unwrap();

        assert_eq!(read_launches(&connection).await.unwrap(), vec![launch]);
    }

    #[tokio::test]
    async fn repeated_launches_accumulate() {
        let connection = in_memory_connection(&SCHEMA).await;
        write_launch(&connection, &firefox_on_web(5)).await.unwrap();
        write_launch(&connection, &firefox_on_web(0)).await.unwrap();

        assert_eq!(read_launches(&connection).await.unwrap().len(), 2);
    }
}
