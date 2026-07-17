//! Subprocess-backed widget data. Mirrors the contract of Odin's
//! `ReusableComponent/ScriptDataSource.qml`: spawn a Nushell script, collect its
//! stdout, and parse the one-JSON-object-per-run output into typed fields.
//!
//! The Nu scripts in Odin's `scripts/` dir are framework-agnostic — they emit JSON
//! and are reused verbatim here via `std::process::Command` (async via tokio).

use std::path::PathBuf;

use serde::Deserialize;

/// The subset of the widget JSON contract this spike models. `battery.nu` emits:
/// `{ icon, label, tooltip, level, value: { gauge: { percent, unit } } }`
/// or `{ icon, label, tooltip, available: false }` when no battery is present.
#[derive(Debug, Clone, Deserialize)]
pub struct WidgetData {
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub label: String,
    /// Script-reported availability; defaults to true when the key is absent
    /// (matches ScriptDataSource.qml `available ?? true`).
    #[serde(default = "default_true")]
    pub available: bool,
    #[serde(default = "default_ok")]
    pub level: String,
    #[serde(default)]
    pub value: Option<ValueFacet>,
}

/// The single-key tagged-union `value` facet. The spike only needs `gauge`.
#[derive(Debug, Clone, Deserialize)]
pub struct ValueFacet {
    pub gauge: Option<Gauge>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Gauge {
    pub percent: i64,
    #[serde(default = "default_unit")]
    pub unit: String,
}

fn default_true() -> bool {
    true
}
fn default_ok() -> String {
    "ok".to_string()
}
fn default_unit() -> String {
    "%".to_string()
}

impl WidgetData {
    pub fn percent(&self) -> Option<i64> {
        self.value.as_ref()?.gauge.as_ref().map(|g| g.percent)
    }
}

/// Resolve the scripts directory: `$ODIN_SCRIPTS_DIR` or the repo default.
fn scripts_dir() -> PathBuf {
    std::env::var_os("ODIN_SCRIPTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/wvhulle/Code/odin/scripts"))
}

/// Run `nu <scripts_dir>/<name>.nu <args...>` and parse its stdout as `WidgetData`.
///
/// Mirrors `Scripts.qml`'s `["nu", path, ...args]` command shape and
/// ScriptDataSource's parse-or-fallback: on a JSON parse failure the raw trimmed
/// stdout becomes the `label`, exactly as the QML does, so a broken script degrades
/// to a text label instead of a crash.
pub async fn run(name: &str, args: &[&str]) -> Result<WidgetData, String> {
    let path = scripts_dir().join(format!("{name}.nu"));
    let output = tokio::process::Command::new("nu")
        .arg(&path)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("spawn nu {}: {e}", path.display()))?;

    let raw = String::from_utf8_lossy(&output.stdout);
    let trimmed = raw.trim();
    match serde_json::from_str::<WidgetData>(trimmed) {
        Ok(data) => Ok(data),
        Err(_) => Ok(WidgetData {
            icon: String::new(),
            label: trimmed.to_string(),
            available: true,
            level: "ok".to_string(),
            value: None,
        }),
    }
}
