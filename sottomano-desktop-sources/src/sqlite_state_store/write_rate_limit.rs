use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::sqlite_state_store::{Columns, StoreError, connect, with_table};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS rate_limit_backoff (
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
    let stored = match connect(&[CREATE_TABLE]).await {
        Ok(conn) => read_backoff(&conn, source).await,
        Err(error) => Err(error),
    };
    match stored {
        Ok(backoff) => Some(backoff),
        Err(StoreError::NoRow) => None,
        Err(e) => {
            tracing::warn!("rate limit backoff: load failed for {source}: {e}");
            None
        }
    }
}

pub async fn save(source: &'static str, backoff: &Backoff) {
    with_table(
        &format!("rate limit backoff: save failed for {source}"),
        &[CREATE_TABLE],
        async |conn| write_backoff(conn, source, backoff).await,
    )
    .await;
}

pub async fn clear(source: &'static str) {
    with_table(
        &format!("rate limit backoff: clear failed for {source}"),
        &[CREATE_TABLE],
        async |conn| clear_backoff(conn, source).await,
    )
    .await;
}

async fn read_backoff(conn: &turso::Connection, source: &str) -> Result<Backoff, StoreError> {
    let mut rows = conn
        .query(
            "SELECT retry_after_unix, attempts, status, message
             FROM rate_limit_backoff WHERE source = ?1",
            (source.to_string(),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let row = rows.next().await?.ok_or(StoreError::NoRow)?;
    Ok(Backoff {
        retry_after_unix: columns.integer(&row, "retry_after_unix")?,
        attempts: u32::try_from(columns.integer(&row, "attempts")?).unwrap_or(u32::MAX),
        status: u16::try_from(columns.integer(&row, "status")?).unwrap_or(u16::MAX),
        message: columns.optional_text(&row, "message"),
    })
}

async fn write_backoff(
    conn: &turso::Connection,
    source: &str,
    backoff: &Backoff,
) -> Result<(), StoreError> {
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

async fn clear_backoff(conn: &turso::Connection, source: &str) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM rate_limit_backoff WHERE source = ?1",
        (source.to_string(),),
    )
    .await?;
    Ok(())
}

#[must_use]
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::{Backoff, CREATE_TABLE, read_backoff, write_backoff};
    use crate::sqlite_state_store::in_memory_connection;

    fn backoff() -> Backoff {
        Backoff {
            retry_after_unix: 1_700_000_000,
            attempts: 3,
            status: 429,
            message: Some("slow down".to_owned()),
        }
    }

    #[tokio::test]
    async fn backoff_survives_a_round_trip() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        write_backoff(&conn, "claude", &backoff()).await.unwrap();

        assert_eq!(read_backoff(&conn, "claude").await.unwrap(), backoff());
    }
}
