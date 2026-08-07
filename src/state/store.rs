use std::{env, error::Error, fs, ops::Deref, path::PathBuf};

use tokio::sync::{Mutex, MutexGuard, OnceCell};

use super::{idle, launches, panel, power};

pub(crate) type BoxError = Box<dyn Error + Send + Sync>;

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

pub(crate) async fn open() -> Result<Handle, BoxError> {
    let guard = LOCK.lock().await;
    let store = STORE.get_or_try_init(build).await?;
    Ok(Handle {
        _guard: guard,
        connection: store.connection.clone(),
    })
}

async fn build() -> Result<Store, BoxError> {
    let Some(path) = db_path() else {
        return Err("no state directory".into());
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

pub(crate) fn text(row: &turso::Row, index: usize) -> Result<String, BoxError> {
    row.get_value(index)?
        .as_text()
        .map(ToString::to_string)
        .ok_or_else(|| format!("column {index} is not text").into())
}

pub(crate) fn integer(row: &turso::Row, index: usize) -> Result<i64, BoxError> {
    row.get_value(index)?
        .as_integer()
        .copied()
        .ok_or_else(|| format!("column {index} is not an integer").into())
}
