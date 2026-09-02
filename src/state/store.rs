use std::{env, fmt, fs, io, ops::Deref, path::PathBuf};

use tokio::sync::{Mutex, MutexGuard, OnceCell};

use super::{idle, launches, panel, power, rate_limit};

#[derive(Debug)]
pub(crate) enum StoreError {
    MissingStateDirectory,
    CreateDirectory(io::Error),
    Database(turso::Error),
    Column(ColumnError),
    UnknownSetting(String),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ColumnError {
    NotText { index: usize },
    NotInteger { index: usize },
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
            Self::UnknownSetting(key) => write!(f, "unknown setting {key}"),
        }
    }
}

impl fmt::Display for ColumnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotText { index } => write!(f, "column {index} is not text"),
            Self::NotInteger { index } => write!(f, "column {index} is not an integer"),
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
    connection.execute(launches::CREATE_TABLE, ()).await?;
    connection.execute(panel::CREATE_TABLE, ()).await?;
    connection.execute(idle::CREATE_TABLE, ()).await?;
    connection.execute(power::DROP_LEGACY_TABLE, ()).await?;
    connection.execute(power::CREATE_TABLE, ()).await?;
    connection.execute(rate_limit::CREATE_TABLE, ()).await?;
    Ok(Store {
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

pub(crate) fn text(row: &turso::Row, index: usize) -> Result<String, StoreError> {
    let value = row.get_value(index)?;
    let Some(text) = value.as_text() else {
        return Err(ColumnError::NotText { index }.into());
    };
    Ok(text.clone())
}

pub(crate) fn optional_text(row: &turso::Row, index: usize) -> Result<Option<String>, StoreError> {
    Ok(row.get_value(index)?.as_text().cloned())
}

pub(crate) fn integer(row: &turso::Row, index: usize) -> Result<i64, StoreError> {
    let value = row.get_value(index)?;
    let Some(integer) = value.as_integer() else {
        return Err(ColumnError::NotInteger { index }.into());
    };
    Ok(*integer)
}
