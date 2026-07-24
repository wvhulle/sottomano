//! On-disk persistence for the quick-launch [`LaunchTracker`], backed by Turso
//! (pure-Rust SQLite) via its local-file API.
//!
//! The database lives at `$XDG_STATE_HOME/odin-rs/launch-frequency.db`. The
//! model is loaded from it once at startup and each counted launch is written
//! back as an upsert of the absolute count, so the table mirrors the in-memory
//! model rather than re-incrementing. Both operations are async (Turso runs on
//! the tokio runtime) and run as fire-and-forget iced tasks off the render path;
//! any error degrades to an empty load / a skipped write, logged.

use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};

use crate::data_sources::launch_tracker::{Buckets, LaunchDelta, Record};

/// Drop launch entries not used within this window, so the store stays bounded
/// and abandoned apps disappear from quick-launch (odin's `FREQUENCY_TTL`).
const TTL_DAYS: i64 = 60;

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS launches (
    workspace TEXT NOT NULL,
    app_id TEXT NOT NULL,
    count INTEGER NOT NULL,
    last_used TEXT NOT NULL,
    PRIMARY KEY (workspace, app_id)
)";

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Load the persisted model, pruning entries past the TTL first. Degrades to an
/// empty model on any error (absent DB, unreadable state dir).
pub async fn load() -> Buckets {
    match try_load().await {
        Ok(buckets) => buckets,
        Err(e) => {
            eprintln!("launch store: load failed: {e}");
            Buckets::new()
        }
    }
}

/// Persist one counted launch as an upsert of its absolute count and time.
pub async fn record(delta: LaunchDelta) {
    if let Err(e) = try_record(&delta).await {
        eprintln!("launch store: record failed: {e}");
    }
}

async fn open() -> Result<turso::Connection, BoxError> {
    let Some(path) = db_path() else {
        return Err("no state directory".into());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let conn = db.connect()?;
    conn.execute(CREATE_TABLE, ()).await?;
    Ok(conn)
}

async fn try_load() -> Result<Buckets, BoxError> {
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
        let count = integer(&row, 2)?.max(0) as u64;
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

async fn try_record(delta: &LaunchDelta) -> Result<(), BoxError> {
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

fn text(row: &turso::Row, index: usize) -> Result<String, BoxError> {
    row.get_value(index)?
        .as_text()
        .map(ToString::to_string)
        .ok_or_else(|| format!("column {index} is not text").into())
}

fn integer(row: &turso::Row, index: usize) -> Result<i64, BoxError> {
    row.get_value(index)?
        .as_integer()
        .copied()
        .ok_or_else(|| format!("column {index} is not an integer").into())
}

/// odin-rs's own launch-frequency database
/// (`$XDG_STATE_HOME/odin-rs/launch-frequency.db`).
fn db_path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state.join("odin-rs/launch-frequency.db"))
}
