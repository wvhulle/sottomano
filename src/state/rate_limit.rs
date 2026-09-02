use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::state::{StoreError, integer, open, optional_text};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS rate_limit_backoff (
    source TEXT PRIMARY KEY,
    retry_after_unix INTEGER NOT NULL,
    attempts INTEGER NOT NULL,
    status INTEGER NOT NULL,
    message TEXT
)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backoff {
    pub retry_after_unix: i64,
    pub attempts: u32,
    pub status: u16,
    pub message: Option<String>,
}

impl Backoff {
    pub fn remaining(&self) -> Option<Duration> {
        let seconds = self.retry_after_unix.checked_sub(now_unix())?;
        u64::try_from(seconds)
            .ok()
            .filter(|s| *s > 0)
            .map(Duration::from_secs)
    }
}

pub async fn load(source: &'static str) -> Option<Backoff> {
    match try_load(source).await {
        Ok(backoff) => backoff,
        Err(e) => {
            tracing::warn!("rate limit backoff: load failed for {source}: {e}");
            None
        }
    }
}

pub async fn save(source: &'static str, backoff: &Backoff) {
    if let Err(e) = try_save(source, backoff).await {
        tracing::warn!("rate limit backoff: save failed for {source}: {e}");
    }
}

pub async fn clear(source: &'static str) {
    if let Err(e) = try_clear(source).await {
        tracing::warn!("rate limit backoff: clear failed for {source}: {e}");
    }
}

async fn try_load(source: &str) -> Result<Option<Backoff>, StoreError> {
    let conn = open().await?;
    let mut rows = conn
        .query(
            "SELECT retry_after_unix, attempts, status, message
             FROM rate_limit_backoff WHERE source = ?1",
            (source.to_string(),),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(Some(Backoff {
        retry_after_unix: integer(&row, 0)?,
        attempts: u32::try_from(integer(&row, 1)?).unwrap_or(u32::MAX),
        status: u16::try_from(integer(&row, 2)?).unwrap_or(u16::MAX),
        message: optional_text(&row, 3)?,
    }))
}

async fn try_save(source: &str, backoff: &Backoff) -> Result<(), StoreError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO rate_limit_backoff (source, retry_after_unix, attempts, status, message)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source)
         DO UPDATE SET retry_after_unix = ?2, attempts = ?3, status = ?4, message = ?5",
        (
            source.to_string(),
            backoff.retry_after_unix,
            i64::from(backoff.attempts),
            i64::from(backoff.status),
            backoff.message.clone(),
        ),
    )
    .await?;
    Ok(())
}

async fn try_clear(source: &str) -> Result<(), StoreError> {
    let conn = open().await?;
    conn.execute(
        "DELETE FROM rate_limit_backoff WHERE source = ?1",
        (source.to_string(),),
    )
    .await?;
    Ok(())
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}
