use std::{collections::HashMap, env, fmt, fs, io, ops::Deref, path::PathBuf, time::Duration};

use tokio::sync::{Mutex, MutexGuard, OnceCell};

use super::{charge_power_curve, dark_mode, idle, launches, panel, power, rate_limit};

#[derive(Debug)]
pub(crate) enum StoreError {
    MissingStateDirectory,
    CreateDirectory(io::Error),
    Database(turso::Error),
    Column(ColumnError),
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

struct Store {
    connection: turso::Connection,
    _database: turso::Database,
}

static STORE: OnceCell<Store> = OnceCell::const_new();
static LOCK: Mutex<()> = Mutex::const_new(());

pub(crate) struct Handle {
    _guard: MutexGuard<'static, ()>,
    connection: turso::Connection,
}

impl Deref for Handle {
    type Target = turso::Connection;

    fn deref(&self) -> &turso::Connection {
        &self.connection
    }
}

pub(crate) async fn open() -> Result<Handle, StoreError> {
    let guard = LOCK.lock().await;
    let store = STORE.get_or_try_init(build).await?;
    Ok(Handle {
        _guard: guard,
        connection: store.connection.clone(),
    })
}

async fn build() -> Result<Store, StoreError> {
    let Some(path) = db_path() else {
        return Err(StoreError::MissingStateDirectory);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    apply_schema(&connection).await?;
    Ok(Store {
        connection,
        _database: database,
    })
}

pub(crate) async fn apply_schema(connection: &turso::Connection) -> Result<(), StoreError> {
    connection.execute(launches::CREATE_TABLE, ()).await?;
    connection.execute(panel::CREATE_TABLE, ()).await?;
    panel::add_later_columns(connection).await?;
    connection.execute(idle::CREATE_TABLE, ()).await?;
    connection.execute(dark_mode::CREATE_TABLE, ()).await?;
    for statement in power::DROP_LEGACY_TABLES {
        connection.execute(statement, ()).await?;
    }
    connection.execute(power::CREATE_TABLE, ()).await?;
    for statement in charge_power_curve::CREATE_TABLES {
        connection.execute(statement, ()).await?;
    }
    connection.execute(rate_limit::CREATE_TABLE, ()).await?;
    panel::adopt_legacy_settings(connection).await?;
    Ok(())
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

    pub(crate) fn optional_text(
        &self,
        row: &turso::Row,
        name: &str,
    ) -> Result<Option<String>, StoreError> {
        match self.value(row, name)? {
            turso::Value::Text(text) => Ok(Some(text)),
            _ => Ok(None),
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

    pub(crate) fn optional_integer(
        &self,
        row: &turso::Row,
        name: &str,
    ) -> Result<Option<i64>, StoreError> {
        match self.value(row, name)? {
            turso::Value::Integer(integer) => Ok(Some(integer)),
            _ => Ok(None),
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

    pub(crate) fn optional_real(
        &self,
        row: &turso::Row,
        name: &str,
    ) -> Result<Option<f64>, StoreError> {
        match self.value(row, name)? {
            turso::Value::Real(real) => Ok(Some(real)),
            turso::Value::Integer(integer) => Ok(Some(integer as f64)),
            _ => Ok(None),
        }
    }

    pub(crate) fn optional_flag(
        &self,
        row: &turso::Row,
        name: &str,
    ) -> Result<Option<bool>, StoreError> {
        Ok(self
            .optional_integer(row, name)?
            .map(|integer| integer != 0))
    }

    pub(crate) fn duration(&self, row: &turso::Row, name: &str) -> Option<Duration> {
        let seconds = self.optional_integer(row, name).ok()??;
        Some(Duration::from_secs(u64::try_from(seconds).unwrap_or(0)))
    }
}
