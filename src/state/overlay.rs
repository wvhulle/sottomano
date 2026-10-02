use crate::state::{Columns, OverlayPreferences, StoreError, open};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS overlay_appearance (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    opacity REAL,
    corner_radius_px REAL
)";

#[derive(Debug, Clone, Copy)]
pub enum OverlaySetting {
    Opacity,
    CornerRadius,
}

impl OverlaySetting {
    fn column(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::CornerRadius => "corner_radius_px",
        }
    }
}

pub async fn save(setting: OverlaySetting, value: f32) -> bool {
    match try_save(setting, value).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("overlay appearance: save {} failed: {e}", setting.column());
            false
        }
    }
}

async fn try_save(setting: OverlaySetting, value: f32) -> Result<(), StoreError> {
    let conn = open().await?;
    write_setting(&conn, setting, value).await
}

async fn write_setting(
    conn: &turso::Connection,
    setting: OverlaySetting,
    value: f32,
) -> Result<(), StoreError> {
    let column = setting.column();
    conn.execute(
        format!(
            "INSERT INTO overlay_appearance (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (f64::from(value),),
    )
    .await?;
    Ok(())
}

pub(crate) async fn read_overlay_preferences(
    conn: &turso::Connection,
) -> Result<OverlayPreferences, StoreError> {
    let mut rows = conn
        .query(
            "SELECT opacity, corner_radius_px FROM overlay_appearance WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut overlays = OverlayPreferences::default();
    let Some(row) = rows.next().await? else {
        return Ok(overlays);
    };
    if let Some(opacity) = columns.optional_real(&row, "opacity")? {
        overlays.opacity = opacity as f32;
    }
    if let Some(radius) = columns.optional_real(&row, "corner_radius_px")? {
        overlays.corner_radius = (radius as f32).into();
    }
    Ok(overlays)
}

#[cfg(test)]
mod tests {
    use super::{CREATE_TABLE, OverlaySetting, read_overlay_preferences, write_setting};

    #[tokio::test]
    async fn every_setting_round_trips() {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute(CREATE_TABLE, ()).await.unwrap();
        write_setting(&conn, OverlaySetting::Opacity, 0.4)
            .await
            .unwrap();
        write_setting(&conn, OverlaySetting::CornerRadius, 9.0)
            .await
            .unwrap();
        let overlays = read_overlay_preferences(&conn).await.unwrap();
        assert!((overlays.opacity - 0.4).abs() < f32::EPSILON);
        assert!((overlays.corner_radius.top_left - 9.0).abs() < f32::EPSILON);
    }
}
