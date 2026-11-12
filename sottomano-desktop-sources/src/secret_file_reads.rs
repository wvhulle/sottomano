use std::{fs, io, path::Path};

use crate::non_empty_text::non_empty;

#[must_use]
pub fn read_secret(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(secret_text) => non_empty(secret_text.trim().to_owned()),
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => None,
        Err(read_failure) => {
            tracing::warn!("secrets: reading {} failed: {read_failure}", path.display());
            None
        }
    }
}
