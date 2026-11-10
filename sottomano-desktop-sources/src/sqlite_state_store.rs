pub mod versioned_schema_migrations;
pub mod write_rate_limit;

use std::{collections::HashMap, env, fs, io, ops::Deref, path::PathBuf, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no state directory")]
    MissingStateDirectory,
    #[error("creating state directory failed: {0}")]
    CreateDirectory(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] turso::Error),
    #[error("{0}")]
    Column(#[from] ColumnError),
    #[error("no matching row")]
    NoRow,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ColumnError {
    #[error("column {name} is absent from the result set")]
    Missing { name: String },
    #[error("column {name} is not text")]
    NotText { name: String },
    #[error("column {name} is not an integer")]
    NotInteger { name: String },
    #[error("column {name} is not a real")]
    NotReal { name: String },
}

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TableConnection {
    connection: turso::Connection,
    _database: turso::Database,
}

impl Deref for TableConnection {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.connection
    }
}

#[cfg(any(test, feature = "test-support"))]
pub async fn migrated_in_memory_store() -> turso::Connection {
    let database = turso::Builder::new_local(":memory:").build().await.unwrap();
    let connection = database.connect().unwrap();
    versioned_schema_migrations::apply_pending_migrations(&connection)
        .await
        .unwrap();
    connection
}

pub async fn with_state_store<T>(
    label: &str,
    work: impl AsyncFnOnce(&turso::Connection) -> Result<T, StoreError>,
) -> Option<T> {
    in_state_store(work)
        .await
        .inspect_err(|error| tracing::warn!("{label}: {error}"))
        .ok()
}

pub async fn in_state_store<T>(
    work: impl AsyncFnOnce(&turso::Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let connection = connect_migrated_store().await?;
    work(&connection).await
}

pub async fn upsert_column(
    conn: &turso::Connection,
    table: &str,
    column: &str,
    value: turso::Value,
) -> Result<(), StoreError> {
    conn.execute(
        format!(
            "INSERT INTO {table} (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (value,),
    )
    .await?;
    Ok(())
}

#[must_use]
pub fn real(value: f32) -> turso::Value {
    turso::Value::Real(f64::from(value))
}

#[must_use]
pub fn flag(value: bool) -> turso::Value {
    turso::Value::Integer(i64::from(value))
}

pub async fn connect_migrated_store() -> Result<TableConnection, StoreError> {
    let path = db_path().ok_or(StoreError::MissingStateDirectory)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    versioned_schema_migrations::apply_pending_migrations(&connection).await?;
    Ok(TableConnection {
        connection,
        _database: database,
    })
}

fn db_path() -> Option<PathBuf> {
    let state = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state.join("sottomano/state.db"))
}

pub struct Columns(HashMap<String, usize>);

impl Columns {
    pub fn of(rows: &turso::Rows) -> Self {
        Self(
            rows.column_names()
                .into_iter()
                .enumerate()
                .map(|(index, name)| (name.to_lowercase(), index))
                .collect(),
        )
    }

    fn value(&self, row: &turso::Row, name: &str) -> Result<turso::Value, StoreError> {
        let index =
            self.0
                .get(&name.to_lowercase())
                .copied()
                .ok_or_else(|| ColumnError::Missing {
                    name: name.to_owned(),
                })?;
        Ok(row.get_value(index)?)
    }

    pub fn text(&self, row: &turso::Row, name: &str) -> Result<String, StoreError> {
        match self.value(row, name)? {
            turso::Value::Text(text) => Ok(text),
            _ => Err(ColumnError::NotText {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    #[must_use]
    pub fn optional_text(&self, row: &turso::Row, name: &str) -> Option<String> {
        match self.value(row, name).ok()? {
            turso::Value::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn integer(&self, row: &turso::Row, name: &str) -> Result<i64, StoreError> {
        match self.value(row, name)? {
            turso::Value::Integer(integer) => Ok(integer),
            _ => Err(ColumnError::NotInteger {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    #[must_use]
    pub fn optional_integer(&self, row: &turso::Row, name: &str) -> Option<i64> {
        match self.value(row, name).ok()? {
            turso::Value::Integer(integer) => Some(integer),
            _ => None,
        }
    }

    pub fn real(&self, row: &turso::Row, name: &str) -> Result<f64, StoreError> {
        match self.value(row, name)? {
            turso::Value::Real(real) => Ok(real),
            turso::Value::Integer(integer) => Ok(integer as f64),
            _ => Err(ColumnError::NotReal {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    #[must_use]
    pub fn duration(&self, row: &turso::Row, name: &str) -> Option<Duration> {
        let seconds = u64::try_from(self.optional_integer(row, name)?).ok()?;
        Some(Duration::from_secs(seconds))
    }
}
