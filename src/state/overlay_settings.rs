use crate::state::{Columns, OverlaySettings, StoreError, connect};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS overlay_settings (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    opacity REAL,
    corner_radius_px REAL,
    toast_spatial_layout INTEGER
)";

#[derive(Debug, Clone, Copy)]
pub enum OverlaySetting {
    Opacity,
    CornerRadius,
    ToastSpatialLayout,
}

impl OverlaySetting {
    fn column(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::CornerRadius => "corner_radius_px",
            Self::ToastSpatialLayout => "toast_spatial_layout",
        }
    }
}

pub async fn save(setting: OverlaySetting, value: f32) -> bool {
    put(setting, turso::Value::Real(f64::from(value))).await
}

pub async fn save_toast_spatial_layout(spatial: bool) {
    put(
        OverlaySetting::ToastSpatialLayout,
        turso::Value::Integer(i64::from(spatial)),
    )
    .await;
}

async fn put(setting: OverlaySetting, value: turso::Value) -> bool {
    match try_put(setting, value).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("overlay settings: save {} failed: {e}", setting.column());
            false
        }
    }
}

async fn try_put(setting: OverlaySetting, value: turso::Value) -> Result<(), StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
    write_setting(&conn, setting, value).await
}

async fn write_setting(
    conn: &turso::Connection,
    setting: OverlaySetting,
    value: turso::Value,
) -> Result<(), StoreError> {
    let column = setting.column();
    conn.execute(
        format!(
            "INSERT INTO overlay_settings (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (value,),
    )
    .await?;
    Ok(())
}

pub async fn load() -> OverlaySettings {
    try_load().await.unwrap_or_else(|e| {
        tracing::warn!("overlay settings: load failed: {e}");
        OverlaySettings::default()
    })
}

async fn try_load() -> Result<OverlaySettings, StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
    read_overlay_settings(&conn).await
}

async fn read_overlay_settings(conn: &turso::Connection) -> Result<OverlaySettings, StoreError> {
    let mut rows = conn
        .query(
            "SELECT opacity, corner_radius_px, toast_spatial_layout
             FROM overlay_settings WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut overlays = OverlaySettings::default();
    let Some(row) = rows.next().await? else {
        return Ok(overlays);
    };
    if let Some(opacity) = columns.optional_real(&row, "opacity") {
        overlays.opacity = opacity as f32;
    }
    if let Some(radius) = columns.optional_real(&row, "corner_radius_px") {
        overlays.corner_radius = (radius as f32).into();
    }
    if let Some(spatial) = columns.optional_flag(&row, "toast_spatial_layout") {
        overlays.toast_spatial_layout = spatial;
    }
    Ok(overlays)
}

#[cfg(test)]
mod tests {
    use super::{CREATE_TABLE, OverlaySetting, read_overlay_settings, write_setting};

    #[tokio::test]
    async fn every_setting_round_trips() {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        for (setting, value) in [
            (OverlaySetting::Opacity, turso::Value::Real(0.4)),
            (OverlaySetting::CornerRadius, turso::Value::Real(9.0)),
            (OverlaySetting::ToastSpatialLayout, turso::Value::Integer(0)),
        ] {
            write_setting(&conn, setting, value).await.unwrap();
        }
        let overlays = read_overlay_settings(&conn).await.unwrap();
        assert!((overlays.opacity - 0.4).abs() < f32::EPSILON);
        assert!((overlays.corner_radius.top_left - 9.0).abs() < f32::EPSILON);
        assert!(!overlays.toast_spatial_layout);
    }
}
