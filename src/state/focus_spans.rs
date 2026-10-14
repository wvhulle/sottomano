use chrono::{DateTime, TimeDelta, Utc};

use crate::{
    data::source::focus_ledger::{
        FocusSpan, FocusUnit, LoadedSpans, OpenedSpan, PresenceSpan, SpanWrite, StoredFacet,
    },
    state::{Columns, StoreError, with_table},
};

const RETENTION: TimeDelta = TimeDelta::days(400);

pub const SCHEMA: [&str; 7] = [
    "DROP TABLE IF EXISTS focus_span_facets",
    "DROP TABLE IF EXISTS focus_spans",
    "CREATE TABLE IF NOT EXISTS window_focus_spans (
        id INTEGER PRIMARY KEY,
        window_id INTEGER NOT NULL,
        started_at INTEGER NOT NULL,
        ended_at INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS window_focus_spans_by_start ON window_focus_spans (started_at)",
    "CREATE TABLE IF NOT EXISTS window_focus_span_facets (
        span_id INTEGER NOT NULL REFERENCES window_focus_spans (id),
        facet TEXT NOT NULL,
        value TEXT NOT NULL,
        PRIMARY KEY (span_id, facet)
    )",
    "CREATE TABLE IF NOT EXISTS presence_spans (
        id INTEGER PRIMARY KEY,
        started_at INTEGER NOT NULL,
        ended_at INTEGER NOT NULL,
        state TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS presence_spans_by_start ON presence_spans (started_at)",
];

pub async fn load_since(since: DateTime<Utc>) -> LoadedSpans {
    with_table("focus spans: load failed", &SCHEMA, async |conn| {
        forget_before(conn, Utc::now() - RETENTION).await?;
        read_spans(conn, since).await
    })
    .await
    .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    OpenedFocus(OpenedSpan),
    OpenedPresence(OpenedSpan),
    Extended,
    Failed,
}

pub async fn apply(write: SpanWrite) -> Written {
    with_table("focus spans: write failed", &SCHEMA, async |conn| {
        write_span(conn, &write).await
    })
    .await
    .unwrap_or(Written::Failed)
}

fn seconds(at: DateTime<Utc>) -> i64 {
    at.timestamp()
}

fn at_seconds(seconds: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(seconds, 0)
}

async fn forget_before(conn: &turso::Connection, cutoff: DateTime<Utc>) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM window_focus_span_facets WHERE span_id IN (SELECT id FROM window_focus_spans \
         WHERE started_at < ?1)",
        (seconds(cutoff),),
    )
    .await?;
    conn.execute(
        "DELETE FROM window_focus_spans WHERE started_at < ?1",
        (seconds(cutoff),),
    )
    .await?;
    conn.execute(
        "DELETE FROM presence_spans WHERE started_at < ?1",
        (seconds(cutoff),),
    )
    .await?;
    Ok(())
}

struct FacetRow {
    id: i64,
    window: u64,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    unit: Option<FocusUnit>,
}

async fn read_spans(
    conn: &turso::Connection,
    since: DateTime<Utc>,
) -> Result<LoadedSpans, StoreError> {
    let mut rows = conn
        .query(
            "SELECT s.id, s.window_id, s.started_at, s.ended_at, f.facet, f.value
             FROM window_focus_spans s LEFT JOIN window_focus_span_facets f ON f.span_id = s.id
             WHERE s.ended_at >= ?1 ORDER BY s.started_at, s.id",
            (seconds(since),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut facet_rows = Vec::new();
    while let Some(row) = rows.next().await? {
        let stored = columns
            .optional_text(&row, "facet")
            .zip(columns.optional_text(&row, "value"))
            .map(|(facet, value)| StoredFacet { facet, value });
        let (Some(started_at), Some(ended_at)) = (
            at_seconds(columns.integer(&row, "started_at")?),
            at_seconds(columns.integer(&row, "ended_at")?),
        ) else {
            continue;
        };
        facet_rows.push(FacetRow {
            id: columns.integer(&row, "id")?,
            window: columns.integer(&row, "window_id")?.cast_unsigned(),
            started_at,
            ended_at,
            unit: stored.and_then(|stored| FocusUnit::try_from(stored).ok()),
        });
    }
    let focus = facet_rows
        .chunk_by(|previous, next| previous.id == next.id)
        .filter_map(|rows| {
            let first = rows.first()?;
            Some(FocusSpan {
                window: first.window,
                units: rows.iter().filter_map(|row| row.unit.clone()).collect(),
                started_at: first.started_at,
                ended_at: first.ended_at,
            })
        })
        .collect();

    let mut rows = conn
        .query(
            "SELECT id, started_at, ended_at, state FROM presence_spans WHERE ended_at >= ?1 \
             ORDER BY started_at, id",
            (seconds(since),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut presence = Vec::new();
    while let Some(row) = rows.next().await? {
        let (Some(started_at), Some(ended_at), Ok(state)) = (
            at_seconds(columns.integer(&row, "started_at")?),
            at_seconds(columns.integer(&row, "ended_at")?),
            columns.text(&row, "state")?.parse(),
        ) else {
            continue;
        };
        presence.push(PresenceSpan {
            state,
            started_at,
            ended_at,
        });
    }
    Ok(LoadedSpans { focus, presence })
}

async fn inserted_id(
    conn: &turso::Connection,
    statement: &str,
    params: impl turso::IntoParams,
) -> Result<i64, StoreError> {
    let mut rows = conn.query(statement, params).await?;
    let columns = Columns::of(&rows);
    let row = rows.next().await?.ok_or(StoreError::NoRow)?;
    columns.integer(&row, "id")
}

async fn write_span(conn: &turso::Connection, write: &SpanWrite) -> Result<Written, StoreError> {
    match write {
        SpanWrite::OpenFocus(span) => {
            let id = inserted_id(
                conn,
                "INSERT INTO window_focus_spans (window_id, started_at, ended_at) VALUES (?1, ?2, \
                 ?3) RETURNING id",
                (
                    span.window.cast_signed(),
                    seconds(span.started_at),
                    seconds(span.ended_at),
                ),
            )
            .await?;
            for stored in span.units.iter().map(StoredFacet::from) {
                conn.execute(
                    "INSERT INTO window_focus_span_facets (span_id, facet, value) VALUES (?1, ?2, \
                     ?3)",
                    (id, stored.facet, stored.value),
                )
                .await?;
            }
            Ok(Written::OpenedFocus(OpenedSpan {
                id,
                started_at: span.started_at,
            }))
        }
        SpanWrite::OpenPresence(span) => {
            let id = inserted_id(
                conn,
                "INSERT INTO presence_spans (started_at, ended_at, state) VALUES (?1, ?2, ?3) \
                 RETURNING id",
                (
                    seconds(span.started_at),
                    seconds(span.ended_at),
                    span.state.stored_name(),
                ),
            )
            .await?;
            Ok(Written::OpenedPresence(OpenedSpan {
                id,
                started_at: span.started_at,
            }))
        }
        SpanWrite::ExtendFocus { id, ended_at } => {
            conn.execute(
                "UPDATE window_focus_spans SET ended_at = ?1 WHERE id = ?2",
                (seconds(*ended_at), *id),
            )
            .await?;
            Ok(Written::Extended)
        }
        SpanWrite::ExtendPresence { id, ended_at } => {
            conn.execute(
                "UPDATE presence_spans SET ended_at = ?1 WHERE id = ?2",
                (seconds(*ended_at), *id),
            )
            .await?;
            Ok(Written::Extended)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::TimeDelta;

    use super::*;
    use crate::data::source::focus_ledger::Presence;

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        for statement in SCHEMA {
            conn.execute(statement, ()).await.unwrap();
        }
        conn
    }

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn focus_span(minute: i64) -> FocusSpan {
        FocusSpan {
            window: 42,
            units: vec![
                FocusUnit::App("kitty".to_owned()),
                FocusUnit::Project(PathBuf::from("/code/vetrata")),
            ],
            started_at: at(minute),
            ended_at: at(minute),
        }
    }

    #[tokio::test]
    async fn spans_with_facets_round_trip_and_extend_in_place() {
        let conn = connection().await;
        let Written::OpenedFocus(focus) = write_span(&conn, &SpanWrite::OpenFocus(focus_span(0)))
            .await
            .unwrap()
        else {
            panic!("focus span not opened");
        };
        let presence = PresenceSpan {
            state: Presence::ScreenOff,
            started_at: at(0),
            ended_at: at(0),
        };
        let Written::OpenedPresence(opened_presence) =
            write_span(&conn, &SpanWrite::OpenPresence(presence.clone()))
                .await
                .unwrap()
        else {
            panic!("presence span not opened");
        };
        write_span(
            &conn,
            &SpanWrite::ExtendFocus {
                id: focus.id,
                ended_at: at(5),
            },
        )
        .await
        .unwrap();
        write_span(
            &conn,
            &SpanWrite::ExtendPresence {
                id: opened_presence.id,
                ended_at: at(3),
            },
        )
        .await
        .unwrap();

        let loaded = read_spans(&conn, at(-1)).await.unwrap();
        assert_eq!(
            loaded.focus,
            vec![FocusSpan {
                ended_at: at(5),
                ..focus_span(0)
            }]
        );
        assert_eq!(
            loaded.presence,
            vec![PresenceSpan {
                ended_at: at(3),
                ..presence
            }]
        );
    }

    #[tokio::test]
    async fn loading_skips_spans_that_ended_before_the_window() {
        let conn = connection().await;
        write_span(&conn, &SpanWrite::OpenFocus(focus_span(-30)))
            .await
            .unwrap();
        write_span(&conn, &SpanWrite::OpenFocus(focus_span(10)))
            .await
            .unwrap();
        let loaded = read_spans(&conn, at(0)).await.unwrap();
        assert_eq!(loaded.focus.len(), 1);
        assert_eq!(loaded.focus[0].started_at, at(10));
    }

    #[tokio::test]
    async fn forgetting_old_spans_takes_their_facets_along() {
        let conn = connection().await;
        write_span(&conn, &SpanWrite::OpenFocus(focus_span(-30)))
            .await
            .unwrap();
        forget_before(&conn, at(0)).await.unwrap();
        let mut rows = conn
            .query(
                "SELECT COUNT(*) AS facets FROM window_focus_span_facets",
                (),
            )
            .await
            .unwrap();
        let columns = Columns::of(&rows);
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(columns.integer(&row, "facets").unwrap(), 0);
    }
}
