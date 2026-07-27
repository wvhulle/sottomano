//! vetrata's single on-disk state database — all persisted vetrata-owned state
//! lives here, at `$XDG_STATE_HOME/vetrata/state.db`, backed by Turso
//! (pure-Rust SQLite) via its local-file API.
//!
//! Everything vetrata itself remembers across runs goes through this one DB:
//! quick-launch frequencies ([`launches`]), panel settings ([`panel`]), and
//! idle-timeout preferences ([`idle`]). Read-only *user config*
//! (`~/.config/vetrata.toml`) is separate and never written here. Every
//! operation is async (Turso runs on the tokio runtime) and is issued as a
//! fire-and-forget iced task off the render path; any error degrades to a
//! default load / a skipped write, logged.

pub mod idle;
pub mod launches;
pub mod panel;

use std::{ops::Deref, path::PathBuf};

use tokio::sync::{Mutex, MutexGuard, OnceCell};

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The one long-lived database, opened once. turso forbids concurrent use of a
/// connection, so all access is serialized behind [`LOCK`]; rapid writes (e.g.
/// scrolling an idle timeout) queue rather than racing.
struct Store {
    connection: turso::Connection,
    _database: turso::Database,
}

static STORE: OnceCell<Store> = OnceCell::const_new();
static LOCK: Mutex<()> = Mutex::const_new(());

/// An exclusive handle to the shared connection: holds the serialization guard
/// for as long as it is alive and derefs to the connection, so a caller uses it
/// like a plain `Connection` while access stays serialized.
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

/// Acquire the shared connection, opening the database (and creating every
/// table) exactly once. Held exclusively until the returned handle is dropped.
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
        std::fs::create_dir_all(parent)?;
    }
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await?;
    let connection = database.connect()?;
    connection.execute(launches::CREATE_TABLE, ()).await?;
    connection.execute(panel::CREATE_TABLE, ()).await?;
    connection.execute(idle::CREATE_TABLE, ()).await?;
    Ok(Store {
        connection,
        _database: database,
    })
}

/// `$XDG_STATE_HOME/vetrata/state.db` (falling back to `~/.local/state`).
fn db_path() -> Option<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
        })?;
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
