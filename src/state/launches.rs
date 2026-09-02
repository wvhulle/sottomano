use chrono::{DateTime, Duration, Utc};

use crate::{
    data::source::launch_tracker::{Buckets, LaunchDelta, Record},
    state::{Columns, StoreError, open},
};

const TTL_DAYS: i64 = 60;

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS launches (
    workspace TEXT NOT NULL,
    app_id TEXT NOT NULL,
    count INTEGER NOT NULL,
    last_used TEXT NOT NULL,
    PRIMARY KEY (workspace, app_id)
)";

pub async fn load() -> Buckets {
    match try_load().await {
        Ok(buckets) => buckets,
        Err(e) => {
            tracing::warn!("launch store: load failed: {e}");
            Buckets::new()
        }
    }
}

pub async fn record(delta: LaunchDelta) {
    if let Err(e) = try_record(&delta).await {
        tracing::warn!("launch store: record failed: {e}");
    }
}

async fn try_load() -> Result<Buckets, StoreError> {
    let conn = open().await?;
    read_buckets(&conn).await
}

async fn read_buckets(conn: &turso::Connection) -> Result<Buckets, StoreError> {
    let cutoff = (Utc::now() - Duration::days(TTL_DAYS)).to_rfc3339();
    conn.execute("DELETE FROM launches WHERE last_used < ?1", (cutoff,))
        .await?;
    let mut rows = conn
        .query(
            "SELECT workspace, app_id, count, last_used FROM launches",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut buckets = Buckets::new();
    while let Some(row) = rows.next().await? {
        let workspace = columns.text(&row, "workspace")?;
        let app_id = columns.text(&row, "app_id")?;
        let count = u64::try_from(columns.integer(&row, "count")?).unwrap_or(0);
        let Ok(last_used) = DateTime::parse_from_rfc3339(&columns.text(&row, "last_used")?) else {
            continue;
        };
        buckets.entry(workspace).or_default().insert(
            app_id,
            Record {
                count,
                last_used: last_used.with_timezone(&Utc),
            },
        );
    }
    Ok(buckets)
}

async fn try_record(delta: &LaunchDelta) -> Result<(), StoreError> {
    let conn = open().await?;
    write_delta(&conn, delta).await
}

async fn write_delta(conn: &turso::Connection, delta: &LaunchDelta) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO launches (workspace, app_id, count, last_used) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(workspace, app_id) DO UPDATE SET count = ?3, last_used = ?4",
        (
            delta.workspace.clone(),
            delta.app_id.clone(),
            i64::try_from(delta.count).unwrap_or(i64::MAX),
            delta.last_used.to_rfc3339(),
        ),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{CREATE_TABLE, LaunchDelta, read_buckets, write_delta};

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn a_delta_survives_a_round_trip() {
        let conn = connection().await;
        let last_used = Utc::now();
        write_delta(
            &conn,
            &LaunchDelta {
                workspace: "web".to_owned(),
                app_id: "firefox".to_owned(),
                count: 7,
                last_used,
            },
        )
        .await
        .unwrap();

        let buckets = read_buckets(&conn).await.unwrap();
        let record = &buckets["web"]["firefox"];
        assert_eq!(record.count, 7);
        assert_eq!(record.last_used.timestamp(), last_used.timestamp());
    }
}
