use chrono::{DateTime, Duration, Utc};

use crate::{
    data::source::launch_tracker::{Buckets, LaunchDelta, Record},
    state::{StoreError, integer, open, text},
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
    let cutoff = (Utc::now() - Duration::days(TTL_DAYS)).to_rfc3339();
    conn.execute("DELETE FROM launches WHERE last_used < ?1", (cutoff,))
        .await?;
    let mut rows = conn
        .query(
            "SELECT workspace, app_id, count, last_used FROM launches",
            (),
        )
        .await?;
    let mut buckets = Buckets::new();
    while let Some(row) = rows.next().await? {
        let workspace = text(&row, 0)?;
        let app_id = text(&row, 1)?;
        let count = u64::try_from(integer(&row, 2)?).unwrap_or(0);
        let Ok(last_used) = DateTime::parse_from_rfc3339(&text(&row, 3)?) else {
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
