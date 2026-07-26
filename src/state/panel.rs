//! Panel-settings persistence: the session-tunable layout knobs (floating on/off
//! and the float offset) the panel-settings group exposes. Stored as a tiny
//! key/value table so new knobs can be added without a migration. Loaded once at
//! startup and written back on each change.

use crate::state::{BoxError, open, text};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS panel_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
)";

const FLOATING: &str = "floating";
const FLOAT_OFFSET: &str = "float_offset_px";

/// The persisted panel settings, if any were saved. Missing keys are `None`, so
/// the caller keeps its config default.
#[derive(Debug, Clone, Copy, Default)]
pub struct PanelSettings {
    pub floating: Option<bool>,
    pub float_offset_px: Option<f32>,
}

/// Load the saved panel settings, degrading to none on any error.
pub async fn load() -> PanelSettings {
    match try_load().await {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("panel settings: load failed: {e}");
            PanelSettings::default()
        }
    }
}

/// Persist the floating toggle.
pub async fn save_floating(floating: bool) {
    put(FLOATING, if floating { "1" } else { "0" }).await;
}

/// Persist the float offset, reporting whether the write succeeded so a caller
/// can confirm the value actually landed.
pub async fn save_float_offset(offset: f32) -> bool {
    put(FLOAT_OFFSET, &offset.to_string()).await
}

async fn put(key: &str, value: &str) -> bool {
    match try_put(key, value).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("panel settings: save {key} failed: {e}");
            false
        }
    }
}

async fn try_put(key: &str, value: &str) -> Result<(), BoxError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO panel_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        (key.to_string(), value.to_string()),
    )
    .await?;
    Ok(())
}

async fn try_load() -> Result<PanelSettings, BoxError> {
    let conn = open().await?;
    let mut rows = conn
        .query("SELECT key, value FROM panel_settings", ())
        .await?;
    let mut settings = PanelSettings::default();
    while let Some(row) = rows.next().await? {
        let key = text(&row, 0)?;
        let value = text(&row, 1)?;
        match key.as_str() {
            FLOATING => settings.floating = Some(value != "0"),
            FLOAT_OFFSET => settings.float_offset_px = value.parse().ok(),
            _ => {}
        }
    }
    Ok(settings)
}
