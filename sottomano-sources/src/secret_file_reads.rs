use std::{fs, path::Path};

use crate::non_empty_text::non_empty;

#[must_use]
pub fn read_secret(path: &Path) -> Option<String> {
    non_empty(fs::read_to_string(path).ok()?.trim().to_owned())
}
