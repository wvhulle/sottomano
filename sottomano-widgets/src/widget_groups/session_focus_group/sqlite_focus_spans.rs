use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use sottomano_desktop_sources::{
    login_session_presence::focus_time_ledger::{
        FocusSpan, FocusUnit, LoadedSpans, PresenceSpan, SpanWrite, StoredFacet,
    },
    sqlite_state_store::{Columns, StoreError, with_state_store},
};

const RETENTION: TimeDelta = TimeDelta::days(400);

const SORTABLE_INSTANT: SecondsFormat = SecondsFormat::Nanos;
const UTC_AS_Z: bool = true;

pub async fn load_since(since: DateTime<Utc>, now: DateTime<Utc>) -> LoadedSpans {
    with_state_store("focus spans: load failed", async |connection| {
        forget_before(connection, now - RETENTION).await?;
        read_spans(connection, since).await
    })
    .await
    .unwrap_or_default()
}

pub async fn record(write: SpanWrite) {
    with_state_store("focus spans: write failed", async |connection| {
        record_span(connection, &write).await
    })
    .await;
}

async fn forget_before(
    connection: &turso::Connection,
    cutoff: DateTime<Utc>,
) -> Result<(), StoreError> {
    let cutoff = cutoff.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z);
    connection
        .execute(
            "DELETE FROM window_focus_span_facets WHERE span_started_at < ?1",
            (cutoff.as_str(),),
        )
        .await?;
    connection
        .execute(
            "DELETE FROM window_focus_spans WHERE started_at < ?1",
            (cutoff.as_str(),),
        )
        .await?;
    connection
        .execute(
            "DELETE FROM presence_spans WHERE started_at < ?1",
            (cutoff.as_str(),),
        )
        .await?;
    Ok(())
}

struct FacetRow {
    window: u64,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    unit: Option<FocusUnit>,
}

async fn read_spans(
    connection: &turso::Connection,
    since: DateTime<Utc>,
) -> Result<LoadedSpans, StoreError> {
    let mut rows = connection
        .query(
            "SELECT s.window_id, s.started_at, s.ended_at, f.facet, f.value
             FROM window_focus_spans s
             LEFT JOIN window_focus_span_facets f
                 ON f.span_started_at = s.started_at
             WHERE s.ended_at >= ?1 ORDER BY s.started_at",
            (since.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut facet_rows = Vec::new();
    while let Some(row) = rows.next().await? {
        let stored = columns
            .optional_text(&row, "facet")
            .zip(columns.optional_blob(&row, "value"))
            .map(|(facet, value)| StoredFacet { facet, value });
        let (Ok(started_at), Ok(ended_at)) = (
            columns.text(&row, "started_at")?.parse(),
            columns.text(&row, "ended_at")?.parse(),
        ) else {
            continue;
        };
        facet_rows.push(FacetRow {
            window: columns.integer(&row, "window_id")?.cast_unsigned(),
            started_at,
            ended_at,
            unit: stored.and_then(|stored| {
                FocusUnit::try_from(stored)
                    .inspect_err(|facet_failure| {
                        tracing::warn!("focus spans: skipping stored facet: {facet_failure}");
                    })
                    .ok()
            }),
        });
    }
    let window_focus_spans = facet_rows
        .chunk_by(|previous, next| previous.started_at == next.started_at)
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

    let mut rows = connection
        .query(
            "SELECT started_at, ended_at, state FROM presence_spans WHERE ended_at >= ?1 ORDER BY \
             started_at",
            (since.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z),),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut presence_spans = Vec::new();
    while let Some(row) = rows.next().await? {
        let (Ok(started_at), Ok(ended_at), Ok(state)) = (
            columns.text(&row, "started_at")?.parse(),
            columns.text(&row, "ended_at")?.parse(),
            columns.text(&row, "state")?.parse(),
        ) else {
            continue;
        };
        presence_spans.push(PresenceSpan {
            state,
            started_at,
            ended_at,
        });
    }
    Ok(LoadedSpans {
        window_focus_spans,
        presence_spans,
    })
}

async fn record_span(connection: &turso::Connection, write: &SpanWrite) -> Result<(), StoreError> {
    match write {
        SpanWrite::RecordWindowFocusSpan(span) => {
            let started_at = span.started_at.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z);
            connection
                .execute(
                    "INSERT INTO window_focus_spans (started_at, window_id, ended_at) VALUES (?1, \
                     ?2, ?3) ON CONFLICT (started_at) DO UPDATE SET ended_at = max(ended_at, \
                     excluded.ended_at)",
                    (
                        started_at.as_str(),
                        span.window.cast_signed(),
                        span.ended_at.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z),
                    ),
                )
                .await?;
            for stored in span.units.iter().map(StoredFacet::from) {
                connection
                    .execute(
                        "INSERT OR IGNORE INTO window_focus_span_facets (span_started_at, facet, \
                         value) VALUES (?1, ?2, ?3)",
                        (started_at.as_str(), stored.facet, stored.value),
                    )
                    .await?;
            }
        }
        SpanWrite::RecordPresenceSpan(span) => {
            connection
                .execute(
                    "INSERT INTO presence_spans (started_at, ended_at, state) VALUES (?1, ?2, ?3) \
                     ON CONFLICT (started_at) DO UPDATE SET ended_at = max(ended_at, \
                     excluded.ended_at)",
                    (
                        span.started_at.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z),
                        span.ended_at.to_rfc3339_opts(SORTABLE_INSTANT, UTC_AS_Z),
                        span.state.stored_name(),
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::TimeDelta;
    use sottomano_desktop_sources::{
        login_session_presence::focus_time_ledger::Presence,
        sqlite_state_store::migrated_in_memory_store,
    };

    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn focus_span(minute: i64) -> FocusSpan {
        FocusSpan {
            window: 42,
            units: vec![
                FocusUnit::App("kitty".to_owned()),
                FocusUnit::Project(PathBuf::from("/code/sottomano")),
            ],
            started_at: at(minute),
            ended_at: at(minute),
        }
    }

    async fn recorded(connection: &turso::Connection, writes: &[SpanWrite]) {
        for write in writes {
            record_span(connection, write).await.unwrap();
        }
    }

    #[tokio::test]
    async fn recording_a_span_again_keeps_its_latest_end_in_any_order() {
        let connection = migrated_in_memory_store().await;
        let presence = PresenceSpan {
            state: Presence::ScreenOff,
            started_at: at(0),
            ended_at: at(0),
        };
        recorded(
            &connection,
            &[
                SpanWrite::RecordWindowFocusSpan(focus_span(0)),
                SpanWrite::RecordWindowFocusSpan(FocusSpan {
                    ended_at: at(5),
                    ..focus_span(0)
                }),
                SpanWrite::RecordPresenceSpan(PresenceSpan {
                    ended_at: at(3),
                    ..presence.clone()
                }),
                SpanWrite::RecordPresenceSpan(presence.clone()),
            ],
        )
        .await;

        let loaded = read_spans(&connection, at(-1)).await.unwrap();
        assert_eq!(
            loaded.window_focus_spans,
            vec![FocusSpan {
                ended_at: at(5),
                ..focus_span(0)
            }]
        );
        assert_eq!(
            loaded.presence_spans,
            vec![PresenceSpan {
                ended_at: at(3),
                ..presence
            }]
        );
    }

    #[tokio::test]
    async fn spans_starting_within_the_same_second_stay_distinct() {
        let connection = migrated_in_memory_store().await;
        let first = focus_span(0);
        let second = FocusSpan {
            window: 43,
            started_at: at(0) + TimeDelta::milliseconds(300),
            ended_at: at(0) + TimeDelta::milliseconds(300),
            ..focus_span(0)
        };
        recorded(
            &connection,
            &[
                SpanWrite::RecordWindowFocusSpan(first.clone()),
                SpanWrite::RecordWindowFocusSpan(second.clone()),
            ],
        )
        .await;
        let loaded = read_spans(&connection, at(-1)).await.unwrap();
        assert_eq!(loaded.window_focus_spans, vec![first, second]);
    }

    #[tokio::test]
    async fn loading_skips_spans_that_ended_before_the_window() {
        let connection = migrated_in_memory_store().await;
        recorded(
            &connection,
            &[
                SpanWrite::RecordWindowFocusSpan(focus_span(-30)),
                SpanWrite::RecordWindowFocusSpan(focus_span(10)),
            ],
        )
        .await;
        let loaded = read_spans(&connection, at(0)).await.unwrap();
        assert_eq!(loaded.window_focus_spans.len(), 1);
        assert_eq!(loaded.window_focus_spans[0].started_at, at(10));
    }

    #[tokio::test]
    async fn forgetting_old_spans_takes_their_facets_along() {
        let connection = migrated_in_memory_store().await;
        recorded(
            &connection,
            &[SpanWrite::RecordWindowFocusSpan(focus_span(-30))],
        )
        .await;
        forget_before(&connection, at(0)).await.unwrap();
        let mut rows = connection
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
