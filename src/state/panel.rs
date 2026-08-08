use crate::state::{Session, StoreError, open, text};

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
const SPATIAL_LAYOUT: &str = "spatial_layout";

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
    put_flag(FLOATING, floating).await;
}

pub async fn save_float_offset(offset: f32) -> bool {
    put(FLOAT_OFFSET, &offset.to_string()).await
}

pub async fn save_blur_background(blur: bool) {
    put_flag(BLUR_BACKGROUND, blur).await;
}

pub async fn save_disconnected_groups(disconnected: bool) {
    put_flag(DISCONNECTED_GROUPS, disconnected).await;
}

pub async fn save_darken_chrome_factor(factor: f32) -> bool {
    put(DARKEN_CHROME_FACTOR, &factor.to_string()).await
}

pub async fn save_dnd(dnd: bool) {
    put_flag(DND, dnd).await;
}

pub async fn save_sound(sound: bool) {
    put_flag(SOUND, sound).await;
}

pub async fn save_spatial_layout(spatial: bool) {
    put_flag(SPATIAL_LAYOUT, spatial).await;
}

async fn put_flag(key: &str, flag: bool) -> bool {
    put(key, if flag { "1" } else { "0" }).await
}

fn parse_flag(value: &str) -> bool {
    value != "0"
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

async fn try_put(key: &str, value: &str) -> Result<(), StoreError> {
    let conn = open().await?;
    conn.execute(
        "INSERT INTO panel_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        (key.to_string(), value.to_string()),
    )
    .await?;
    Ok(())
}

async fn try_load() -> Result<Session, StoreError> {
    let conn = open().await?;
    let mut rows = conn
        .query("SELECT key, value FROM panel_settings", ())
        .await?;
    let mut session = Session::default();
    while let Some(row) = rows.next().await? {
        let key = text(&row, 0)?;
        let value = text(&row, 1)?;
        match key.as_str() {
            FLOATING => session.chrome.floating = parse_flag(&value),
            FLOAT_OFFSET => session.chrome.float_offset_px = value.parse().unwrap_or_default(),
            BLUR_BACKGROUND => session.chrome.blur_background = parse_flag(&value),
            DISCONNECTED_GROUPS => session.chrome.disconnected_groups = parse_flag(&value),
            DARKEN_CHROME_FACTOR => {
                session.chrome.darken_chrome_factor = value.parse().unwrap_or_default();
            }
            DND => session.notifications.dnd = parse_flag(&value),
            SOUND => session.notifications.sound = parse_flag(&value),
            SPATIAL_LAYOUT => session.notifications.spatial_layout = parse_flag(&value),
            _ => {}
        }
    }
    Ok(session)
}
