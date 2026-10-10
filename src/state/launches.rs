use chrono::{DateTime, Duration, Utc};

use crate::{
    data::source::launch_tracker::Launch,
    state::{Columns, StoreError, connect},
};

const TTL_DAYS: i64 = 60;

pub(crate) const SCHEMA: [&str; 2] = [
    "CREATE TABLE IF NOT EXISTS launch_events (
        workspace TEXT NOT NULL,
        app_id TEXT NOT NULL,
        launched_at TEXT NOT NULL
    )",
    "DROP TABLE IF EXISTS launches",
];

pub async fn load() -> Vec<Launch> {
    match try_load().await {
        Ok(launches) => launches,
        Err(e) => {
            tracing::warn!("launch store: load failed: {e}");
            Vec::new()
        }
    }
}

pub async fn record(launch: Launch) {
    if let Err(e) = try_record(&launch).await {
        tracing::warn!("launch store: record failed: {e}");
    }
}

async fn try_load() -> Result<Vec<Launch>, StoreError> {
    let conn = connect(&SCHEMA).await?;
    read_launches(&conn).await
}

async fn read_launches(conn: &turso::Connection) -> Result<Vec<Launch>, StoreError> {
    let cutoff = (Utc::now() - Duration::days(TTL_DAYS)).to_rfc3339();
    conn.execute(
        "DELETE FROM launch_events WHERE launched_at < ?1",
        (cutoff,),
    )
    .await?;
    let mut rows = conn
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

async fn try_record(launch: &Launch) -> Result<(), StoreError> {
    let conn = connect(&SCHEMA).await?;
    write_launch(&conn, launch).await
}

async fn write_launch(conn: &turso::Connection, launch: &Launch) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO launch_events (workspace, app_id, launched_at) VALUES (?1, ?2, ?3)",
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

    use super::{Launch, SCHEMA, read_launches, write_launch};

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        for statement in SCHEMA {
            conn.execute(statement, ()).await.unwrap();
        }
        conn
    }

    fn firefox_on_web(minutes_ago: i64) -> Launch {
        Launch {
            workspace: "web".to_owned(),
            app_id: "firefox".to_owned(),
            launched_at: Utc::now() - Duration::minutes(minutes_ago),
        }
    }

    #[tokio::test]
    async fn a_launch_survives_a_round_trip() {
        let conn = connection().await;
        let launch = firefox_on_web(0);
        write_launch(&conn, &launch).await.unwrap();

        assert_eq!(read_launches(&conn).await.unwrap(), vec![launch]);
    }

    #[tokio::test]
    async fn repeated_launches_accumulate() {
        let conn = connection().await;
        write_launch(&conn, &firefox_on_web(5)).await.unwrap();
        write_launch(&conn, &firefox_on_web(0)).await.unwrap();

        assert_eq!(read_launches(&conn).await.unwrap().len(), 2);
    }
}
