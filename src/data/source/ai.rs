use std::{fs, path::Path};

pub mod claude;
pub mod copilot;
pub mod openrouter;

pub fn read_secret(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}
