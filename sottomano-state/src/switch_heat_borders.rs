use sottomano_sources::sqlite_state_store::{Columns, StoreError, flag, upsert_column, with_table};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS switch_heat_borders (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    shown INTEGER
)";

const SHOWN_BY_DEFAULT: bool = true;

pub async fn load_shown() -> bool {
    with_table(
        "switch heat borders: load failed",
        &[CREATE_TABLE],
        read_shown,
    )
    .await
    .unwrap_or(SHOWN_BY_DEFAULT)
}

pub async fn save_shown(shown: bool) -> bool {
    with_table(
        "switch heat borders: save failed",
        &[CREATE_TABLE],
        async |conn| upsert_column(conn, "switch_heat_borders", "shown", flag(shown)).await,
    )
    .await
    .is_some()
}

async fn read_shown(conn: &turso::Connection) -> Result<bool, StoreError> {
    let mut rows = conn
        .query("SELECT shown FROM switch_heat_borders WHERE id = 0", ())
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(SHOWN_BY_DEFAULT);
    };
    Ok(columns
        .optional_flag(&row, "shown")
        .unwrap_or(SHOWN_BY_DEFAULT))
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::{flag, upsert_column};

    use super::{CREATE_TABLE, read_shown};

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn borders_show_until_turned_off() {
        let conn = connection().await;
        assert!(read_shown(&conn).await.unwrap());
        upsert_column(&conn, "switch_heat_borders", "shown", flag(false))
            .await
            .unwrap();
        assert!(!read_shown(&conn).await.unwrap());
    }
}
