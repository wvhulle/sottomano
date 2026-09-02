use crate::state::{Columns, Session, StoreError, open};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS panel_session (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    corner_radius_px REAL,
    float_offset_px REAL,
    chrome_tint_factor REAL,
    blur_background INTEGER,
    disconnected_groups INTEGER,
    auto_collapse_groups INTEGER,
    dnd INTEGER,
    sound INTEGER,
    spatial_layout INTEGER
)";

const SELECT_SESSION: &str = "SELECT corner_radius_px, float_offset_px, chrome_tint_factor,
        blur_background, disconnected_groups, auto_collapse_groups, dnd, sound, spatial_layout
    FROM panel_session WHERE id = 0";

#[derive(Debug, Clone, Copy)]
enum Setting {
    CornerRadius,
    FloatOffset,
    ChromeTintFactor,
    BlurBackground,
    DisconnectedGroups,
    AutoCollapseGroups,
    Dnd,
    Sound,
    SpatialLayout,
}

impl Setting {
    fn column(self) -> &'static str {
        match self {
            Self::CornerRadius => "corner_radius_px",
            Self::FloatOffset => "float_offset_px",
            Self::ChromeTintFactor => "chrome_tint_factor",
            Self::BlurBackground => "blur_background",
            Self::DisconnectedGroups => "disconnected_groups",
            Self::AutoCollapseGroups => "auto_collapse_groups",
            Self::Dnd => "dnd",
            Self::Sound => "sound",
            Self::SpatialLayout => "spatial_layout",
        }
    }
}

pub async fn load() -> Session {
    try_load().await.unwrap_or_else(|e| {
        tracing::warn!("panel settings: load failed: {e}");
        Session::default()
    })
}

pub async fn save_corner_radius(radius: f32) -> bool {
    put(Setting::CornerRadius, measure(radius)).await
}

pub async fn save_float_offset(offset: f32) -> bool {
    put(Setting::FloatOffset, measure(offset)).await
}

pub async fn save_blur_background(blur: bool) {
    put(Setting::BlurBackground, flag(blur)).await;
}

pub async fn save_disconnected_groups(disconnected: bool) {
    put(Setting::DisconnectedGroups, flag(disconnected)).await;
}

pub async fn save_auto_collapse_groups(collapse: bool) {
    put(Setting::AutoCollapseGroups, flag(collapse)).await;
}

pub async fn save_chrome_tint_factor(factor: f32) -> bool {
    put(Setting::ChromeTintFactor, measure(factor)).await
}

pub async fn save_dnd(dnd: bool) {
    put(Setting::Dnd, flag(dnd)).await;
}

pub async fn save_sound(sound: bool) {
    put(Setting::Sound, flag(sound)).await;
}

pub async fn save_spatial_layout(spatial: bool) {
    put(Setting::SpatialLayout, flag(spatial)).await;
}

fn measure(value: f32) -> turso::Value {
    turso::Value::Real(f64::from(value))
}

fn flag(value: bool) -> turso::Value {
    turso::Value::Integer(i64::from(value))
}

async fn put(setting: Setting, value: turso::Value) -> bool {
    match try_put(setting, value).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("panel settings: save {} failed: {e}", setting.column());
            false
        }
    }
}

async fn try_put(setting: Setting, value: turso::Value) -> Result<(), StoreError> {
    let conn = open().await?;
    write_setting(&conn, setting, value).await
}

async fn write_setting(
    conn: &turso::Connection,
    setting: Setting,
    value: turso::Value,
) -> Result<(), StoreError> {
    let column = setting.column();
    conn.execute(
        format!(
            "INSERT INTO panel_session (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (value,),
    )
    .await?;
    Ok(())
}

async fn try_load() -> Result<Session, StoreError> {
    let conn = open().await?;
    read_session(&conn).await
}

async fn read_session(conn: &turso::Connection) -> Result<Session, StoreError> {
    let mut rows = conn.query(SELECT_SESSION, ()).await?;
    let columns = Columns::of(&rows);
    let mut session = Session::default();
    let Some(row) = rows.next().await? else {
        return Ok(session);
    };
    if let Some(radius) = columns.optional_real(&row, "corner_radius_px")? {
        session.chrome.corner_radius_px = radius as f32;
    }
    if let Some(offset) = columns.optional_real(&row, "float_offset_px")? {
        session.chrome.float_offset_px = offset as f32;
    }
    if let Some(factor) = columns.optional_real(&row, "chrome_tint_factor")? {
        session.chrome.chrome_tint_factor = factor as f32;
    }
    if let Some(blur) = columns.optional_flag(&row, "blur_background")? {
        session.chrome.blur_background = blur;
    }
    if let Some(disconnected) = columns.optional_flag(&row, "disconnected_groups")? {
        session.chrome.disconnected_groups = disconnected;
    }
    if let Some(collapse) = columns.optional_flag(&row, "auto_collapse_groups")? {
        session.chrome.auto_collapse_groups = collapse;
    }
    if let Some(dnd) = columns.optional_flag(&row, "dnd")? {
        session.notifications.dnd = dnd;
    }
    if let Some(sound) = columns.optional_flag(&row, "sound")? {
        session.notifications.sound = sound;
    }
    if let Some(spatial) = columns.optional_flag(&row, "spatial_layout")? {
        session.notifications.spatial_layout = spatial;
    }
    Ok(session)
}

pub(crate) async fn adopt_legacy_settings(conn: &turso::Connection) -> Result<(), StoreError> {
    let mut tables = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'panel_settings'",
            (),
        )
        .await?;
    if tables.next().await?.is_none() {
        return Ok(());
    }

    let mut rows = conn
        .query("SELECT key, value FROM panel_settings", ())
        .await?;
    let columns = Columns::of(&rows);
    let mut session = Session::default();
    while let Some(row) = rows.next().await? {
        let key = columns.text(&row, "key")?;
        let value = columns.text(&row, "value")?;
        let set = value != "0";
        match key.as_str() {
            "corner_radius_px" => {
                session.chrome.corner_radius_px = value.parse().unwrap_or_default();
            }
            "float_offset_px" => session.chrome.float_offset_px = value.parse().unwrap_or_default(),
            "chrome_tint_factor" | "darken_chrome_factor" => {
                session.chrome.chrome_tint_factor = value.parse().unwrap_or_default();
            }
            "blur_background" => session.chrome.blur_background = set,
            "disconnected_groups" => session.chrome.disconnected_groups = set,
            "auto_collapse_groups" => session.chrome.auto_collapse_groups = set,
            "dnd" => session.notifications.dnd = set,
            "sound" => session.notifications.sound = set,
            "spatial_layout" => session.notifications.spatial_layout = set,
            _ => tracing::warn!("panel settings: discarding unknown legacy setting {key}"),
        }
    }
    drop(rows);

    conn.execute(
        "INSERT INTO panel_session (id, corner_radius_px, float_offset_px, chrome_tint_factor,
             blur_background, disconnected_groups, auto_collapse_groups, dnd, sound, \
         spatial_layout)
         VALUES (0, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET corner_radius_px = ?1, float_offset_px = ?2,
             chrome_tint_factor = ?3, blur_background = ?4, disconnected_groups = ?5,
             auto_collapse_groups = ?6, dnd = ?7, sound = ?8, spatial_layout = ?9",
        (
            f64::from(session.chrome.corner_radius_px),
            f64::from(session.chrome.float_offset_px),
            f64::from(session.chrome.chrome_tint_factor),
            i64::from(session.chrome.blur_background),
            i64::from(session.chrome.disconnected_groups),
            i64::from(session.chrome.auto_collapse_groups),
            i64::from(session.notifications.dnd),
            i64::from(session.notifications.sound),
            i64::from(session.notifications.spatial_layout),
        ),
    )
    .await?;
    conn.execute("DROP TABLE panel_settings", ()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CREATE_TABLE, Setting, flag, measure, read_session, write_setting};

    async fn connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn every_setting_round_trips_to_its_own_column() {
        let conn = connection().await;
        for (setting, value) in [
            (Setting::CornerRadius, measure(12.5)),
            (Setting::FloatOffset, measure(3.25)),
            (Setting::ChromeTintFactor, measure(0.75)),
            (Setting::BlurBackground, flag(true)),
            (Setting::DisconnectedGroups, flag(true)),
            (Setting::AutoCollapseGroups, flag(false)),
            (Setting::Dnd, flag(true)),
            (Setting::Sound, flag(false)),
            (Setting::SpatialLayout, flag(false)),
        ] {
            write_setting(&conn, setting, value).await.unwrap();
        }

        let session = read_session(&conn).await.unwrap();
        assert!((session.chrome.corner_radius_px - 12.5).abs() < f32::EPSILON);
        assert!((session.chrome.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!((session.chrome.chrome_tint_factor - 0.75).abs() < f32::EPSILON);
        assert!(session.chrome.blur_background);
        assert!(session.chrome.disconnected_groups);
        assert!(!session.chrome.auto_collapse_groups);
        assert!(session.notifications.dnd);
        assert!(!session.notifications.sound);
        assert!(!session.notifications.spatial_layout);
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = connection().await;
        write_setting(&conn, Setting::Dnd, flag(true))
            .await
            .unwrap();

        let session = read_session(&conn).await.unwrap();
        assert!(session.notifications.dnd);
        assert!(session.chrome.auto_collapse_groups);
        assert!(session.notifications.sound);
        assert!(session.notifications.spatial_layout);
    }
}
