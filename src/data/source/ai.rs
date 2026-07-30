//! AI-provider usage/spend sources, ported from odin's
//! `WidgetGroup/AiUsage.qml` and its `*-usage.nu` scripts: `OpenRouter` spend,
//! Claude subscription/API usage, and GitHub Copilot quota. Each hits a
//! provider HTTP API (or the local `ccusage` CLI) and renders as a bar pill
//! inside [`AiGroup`](crate::widget_groups::ai_group).
//!
//! Unlike a plain [`Source`](crate::data::source::Source), these read a secret
//! from a file whose path is resolved once at startup
//! ([`SecretPaths`](crate::config::resolved::SecretPaths)) and threaded into
//! the poll — hence the separate [`ApiSource`] contract. The file is read on
//! every poll (never cached), so a rotated token is picked up automatically and
//! a truncated mid-write read simply yields `None`.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::data::source::{BarWidget, Level};

pub mod claude;
pub mod copilot;
pub mod http;
pub mod openrouter;

/// A usage source backed by a provider API and a secret file. Mirrors
/// [`Source`](crate::data::source::Source) but its [`poll`](ApiSource::poll)
/// takes the resolved secret-file path (`None` for the keyless Copilot-CLI
/// source, which reads only local `ccusage` data).
pub trait ApiSource: BarWidget + Sized + Send + 'static {
    const NAME: &'static str;
    const INTERVAL: Duration;

    fn poll(key: Option<PathBuf>) -> Option<Self>;
}

pub fn read_secret(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub fn level_from_cost(cost: f64) -> Level {
    if cost >= 20.0 {
        Level::Critical
    } else if cost >= 5.0 {
        Level::Warning
    } else {
        Level::Ok
    }
}
