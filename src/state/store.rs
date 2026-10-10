use std::{collections::HashMap, env, fmt, fs, io, ops::Deref, path::PathBuf, time::Duration};

#[derive(Debug)]
pub(crate) enum StoreError {
    MissingStateDirectory,
    CreateDirectory(io::Error),
    Database(turso::Error),
    Column(ColumnError),
    NoRow,
}

#[derive(Debug, Clone)]
pub(crate) enum ColumnError {
    Missing { name: String },
    NotText { name: String },
    NotInteger { name: String },
    NotReal { name: String },
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::CreateDirectory(e)
    }
}

impl From<turso::Error> for StoreError {
    fn from(e: turso::Error) -> Self {
        Self::Database(e)
    }
}

impl From<ColumnError> for StoreError {
    fn from(e: ColumnError) -> Self {
        Self::Column(e)
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingStateDirectory => write!(f, "no state directory"),
            Self::CreateDirectory(e) => write!(f, "creating state directory failed: {e}"),
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Column(e) => write!(f, "{e}"),
            Self::NoRow => write!(f, "no matching row"),
        }
    }
}

impl fmt::Display for ColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { name } => write!(f, "column {name} is absent from the result set"),
            Self::NotText { name } => write!(f, "column {name} is not text"),
            Self::NotInteger { name } => write!(f, "column {name} is not an integer"),
            Self::NotReal { name } => write!(f, "column {name} is not a real"),
        }
    }
}

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct TableConnection {
    connection: turso::Connection,
    _database: turso::Database,
}

impl Deref for TableConnection {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.connection
    }
}

pub(crate) async fn connect(schema: &[&str]) -> Result<TableConnection, StoreError> {
    let path = db_path().ok_or(StoreError::MissingStateDirectory)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    for statement in schema {
        connection.execute(statement, ()).await?;
    }
    Ok(TableConnection {
        connection,
        _database: database,
    })
}

fn db_path() -> Option<PathBuf> {
    let state = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(state.join("vetrata/state.db"))
}

pub(crate) struct Columns(HashMap<String, usize>);

impl Columns {
    pub(crate) fn of(rows: &turso::Rows) -> Self {
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

    pub(crate) fn text(&self, row: &turso::Row, name: &str) -> Result<String, StoreError> {
        match self.value(row, name)? {
            turso::Value::Text(text) => Ok(text),
            _ => Err(ColumnError::NotText {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    pub(crate) fn optional_text(&self, row: &turso::Row, name: &str) -> Option<String> {
        match self.value(row, name).ok()? {
            turso::Value::Text(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn integer(&self, row: &turso::Row, name: &str) -> Result<i64, StoreError> {
        match self.value(row, name)? {
            turso::Value::Integer(integer) => Ok(integer),
            _ => Err(ColumnError::NotInteger {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    pub(crate) fn optional_integer(&self, row: &turso::Row, name: &str) -> Option<i64> {
        match self.value(row, name).ok()? {
            turso::Value::Integer(integer) => Some(integer),
            _ => None,
        }
    }

    pub(crate) fn real(&self, row: &turso::Row, name: &str) -> Result<f64, StoreError> {
        match self.value(row, name)? {
            turso::Value::Real(real) => Ok(real),
            turso::Value::Integer(integer) => Ok(integer as f64),
            _ => Err(ColumnError::NotReal {
                name: name.to_owned(),
            }
            .into()),
        }
    }

    pub(crate) fn optional_real(&self, row: &turso::Row, name: &str) -> Option<f64> {
        match self.value(row, name).ok()? {
            turso::Value::Real(real) => Some(real),
            turso::Value::Integer(integer) => Some(integer as f64),
            _ => None,
        }
    }

    pub(crate) fn optional_flag(&self, row: &turso::Row, name: &str) -> Option<bool> {
        self.optional_integer(row, name).map(|integer| integer != 0)
    }

    pub(crate) fn duration(&self, row: &turso::Row, name: &str) -> Option<Duration> {
        let seconds = u64::try_from(self.optional_integer(row, name)?).ok()?;
        Some(Duration::from_secs(seconds))
    }
}
