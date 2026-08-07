use crate::state::{BoxError, Session, open, text};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS panel_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
)";

const FLOATING: &str = "floating";
const FLOAT_OFFSET: &str = "float_offset_px";
const BLUR_BACKGROUND: &str = "blur_background";
const DISCONNECTED_GROUPS: &str = "disconnected_groups";
const DARKEN_CHROME_FACTOR: &str = "darken_chrome_factor";
const DND: &str = "dnd";
const SOUND: &str = "sound";

pub async fn load() -> Session {
    match try_load().await {
        Ok(session) => session,
        Err(e) => {
            eprintln!("panel settings: load failed: {e}");
            Session::default()
        }
    }
}

pub async fn save_floating(floating: bool) {
    put(FLOATING, if floating { "1" } else { "0" }).await;
}

pub async fn save_float_offset(offset: f32) -> bool {
    put(FLOAT_OFFSET, &offset.to_string()).await
}

pub async fn save_blur_background(blur: bool) {
    put(BLUR_BACKGROUND, if blur { "1" } else { "0" }).await;
}

pub async fn save_disconnected_groups(disconnected: bool) {
    put(DISCONNECTED_GROUPS, if disconnected { "1" } else { "0" }).await;
}

pub async fn save_darken_chrome_factor(factor: f32) -> bool {
    put(DARKEN_CHROME_FACTOR, &factor.to_string()).await
}

pub async fn save_dnd(dnd: bool) {
    put(DND, if dnd { "1" } else { "0" }).await;
}

pub async fn save_sound(sound: bool) {
    put(SOUND, if sound { "1" } else { "0" }).await;
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

async fn try_load() -> Result<Session, BoxError> {
    let conn = open().await?;
    let mut rows = conn
        .query("SELECT key, value FROM panel_settings", ())
        .await?;
    let mut session = Session::default();
    while let Some(row) = rows.next().await? {
        let key = text(&row, 0)?;
        let value = text(&row, 1)?;
        match key.as_str() {
            FLOATING => session.chrome.floating = value != "0",
            FLOAT_OFFSET => session.chrome.float_offset_px = value.parse().unwrap_or_default(),
            BLUR_BACKGROUND => session.chrome.blur_background = value != "0",
            DISCONNECTED_GROUPS => session.chrome.disconnected_groups = value != "0",
            DARKEN_CHROME_FACTOR => {
                session.chrome.darken_chrome_factor = value.parse().unwrap_or_default();
            }
            DND => session.notifications.dnd = value != "0",
            SOUND => session.notifications.sound = value != "0",
            _ => {}
        }
    }
    Ok(session)
}
