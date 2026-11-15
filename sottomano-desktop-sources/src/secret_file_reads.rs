use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{non_empty_text::non_empty, system_protocol_clients::http_json_client};

#[derive(Debug, Clone, thiserror::Error)]
pub enum SecretUnreadable {
    #[error("secret file {} is missing", path.display())]
    Missing { path: PathBuf },
    #[error("secret file {} is empty", path.display())]
    Empty { path: PathBuf },
    #[error("cannot read secret file {}: {reason}", path.display())]
    Unreadable {
        path: PathBuf,
        reason: Arc<io::Error>,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthenticatedFetchError {
    #[error("{0}")]
    Secret(#[from] SecretUnreadable),
    #[error("{0}")]
    Fetch(#[from] http_json_client::Error),
}

pub fn read_secret(path: &Path) -> Result<String, SecretUnreadable> {
    let secret_text = match fs::read_to_string(path) {
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => {
            return Err(SecretUnreadable::Missing {
                path: path.to_owned(),
            });
        }
        Err(read_failure) => {
            return Err(SecretUnreadable::Unreadable {
                path: path.to_owned(),
                reason: Arc::new(read_failure),
            });
        }
        Ok(secret_text) => secret_text,
    };
    non_empty(secret_text.trim().to_owned()).ok_or_else(|| SecretUnreadable::Empty {
        path: path.to_owned(),
    })
}
