use crate::state::{Columns, PanelSettings, StoreError, WorkspacesPlacement, open, overlay};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS panel_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    float_offset_px REAL,
    frame_tint_factor REAL,
    disconnected_groups INTEGER,
    auto_collapse_groups INTEGER,
    do_not_disturb INTEGER,
    sound INTEGER,
    spatial_layout INTEGER,
    workspaces_in_sidebar INTEGER,
    corner_radius_px REAL,
    drop_shadow INTEGER,
    shadow_softness_px REAL,
    shadow_opacity REAL
)";

const SELECT_PANEL_SETTINGS: &str = "SELECT float_offset_px, frame_tint_factor,
        disconnected_groups, auto_collapse_groups, do_not_disturb, sound, spatial_layout,
        workspaces_in_sidebar, corner_radius_px, drop_shadow, shadow_softness_px,
        shadow_opacity
    FROM panel_preferences WHERE id = 0";

#[derive(Debug, Clone, Copy)]
enum Setting {
    FloatOffset,
    TintFactor,
    DisconnectedGroups,
    AutoCollapseGroups,
    DoNotDisturb,
    Sound,
    SpatialLayout,
    WorkspacesInSidebar,
    CornerRadius,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
}

impl Setting {
    fn column(self) -> &'static str {
        match self {
            Self::CornerRadius => "corner_radius_px",
            Self::DropShadow => "drop_shadow",
            Self::ShadowSoftness => "shadow_softness_px",
            Self::ShadowOpacity => "shadow_opacity",
            Self::FloatOffset => "float_offset_px",
            Self::TintFactor => "frame_tint_factor",
            Self::DisconnectedGroups => "disconnected_groups",
            Self::AutoCollapseGroups => "auto_collapse_groups",
            Self::DoNotDisturb => "do_not_disturb",
            Self::Sound => "sound",
            Self::SpatialLayout => "spatial_layout",
            Self::WorkspacesInSidebar => "workspaces_in_sidebar",
        }
    }
}

pub async fn load() -> PanelSettings {
    try_load().await.unwrap_or_else(|e| {
        tracing::warn!("panel settings: load failed: {e}");
        PanelSettings::default()
    })
}

pub async fn save_float_offset(offset: f32) -> bool {
    put(Setting::FloatOffset, measure(offset)).await
}

pub async fn save_disconnected_groups(disconnected: bool) {
    put(Setting::DisconnectedGroups, flag(disconnected)).await;
}

pub async fn save_auto_collapse_groups(collapse: bool) {
    put(Setting::AutoCollapseGroups, flag(collapse)).await;
}

pub async fn save_workspaces_placement(placement: WorkspacesPlacement) {
    put(
        Setting::WorkspacesInSidebar,
        flag(placement == WorkspacesPlacement::Sidebar),
    )
    .await;
}

pub async fn save_tint_factor(factor: f32) -> bool {
    put(Setting::TintFactor, measure(factor)).await
}

pub async fn save_corner_radius(radius: f32) -> bool {
    put(Setting::CornerRadius, measure(radius)).await
}

pub async fn save_drop_shadow(cast: bool) {
    put(Setting::DropShadow, flag(cast)).await;
}

pub async fn save_shadow_softness(softness: f32) -> bool {
    put(Setting::ShadowSoftness, measure(softness)).await
}

pub async fn save_shadow_opacity(opacity: f32) -> bool {
    put(Setting::ShadowOpacity, measure(opacity)).await
}

pub async fn save_do_not_disturb(dnd: bool) {
    put(Setting::DoNotDisturb, flag(dnd)).await;
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
            "INSERT INTO panel_preferences (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (value,),
    )
    .await?;
    Ok(())
}

async fn try_load() -> Result<PanelSettings, StoreError> {
    let conn = open().await?;
    let mut panel_settings = read_panel_settings(&conn).await?;
    panel_settings.overlays = overlay::read_overlay_preferences(&conn).await?;
    Ok(panel_settings)
}

async fn read_panel_settings(conn: &turso::Connection) -> Result<PanelSettings, StoreError> {
    let mut rows = conn.query(SELECT_PANEL_SETTINGS, ()).await?;
    let columns = Columns::of(&rows);
    let mut panel_settings = PanelSettings::default();
    let Some(row) = rows.next().await? else {
        return Ok(panel_settings);
    };
    if let Some(offset) = columns.optional_real(&row, "float_offset_px")? {
        panel_settings.layout.float_offset_px = offset as f32;
    }
    if let Some(factor) = columns.optional_real(&row, "frame_tint_factor")? {
        panel_settings.layout.tint_factor = factor as f32;
    }
    if let Some(disconnected) = columns.optional_flag(&row, "disconnected_groups")? {
        panel_settings.layout.disconnected_groups = disconnected;
    }
    if let Some(collapse) = columns.optional_flag(&row, "auto_collapse_groups")? {
        panel_settings.layout.auto_collapse_groups = collapse;
    }
    if let Some(dnd) = columns.optional_flag(&row, "do_not_disturb")? {
        panel_settings.toasts.do_not_disturb = dnd;
    }
    if let Some(sound) = columns.optional_flag(&row, "sound")? {
        panel_settings.toasts.sound = sound;
    }
    if let Some(spatial) = columns.optional_flag(&row, "spatial_layout")? {
        panel_settings.toasts.spatial_layout = spatial;
    }
    if let Some(in_sidebar) = columns.optional_flag(&row, "workspaces_in_sidebar")? {
        panel_settings.layout.workspaces_placement = if in_sidebar {
            WorkspacesPlacement::Sidebar
        } else {
            WorkspacesPlacement::BottomPanel
        };
    }
    let layout = &mut panel_settings.layout;
    if let Some(radius) = columns.optional_real(&row, "corner_radius_px")? {
        layout.corner_radius = (radius as f32).into();
    }
    if let Some(cast) = columns.optional_flag(&row, "drop_shadow")? {
        layout.shadow.cast = cast;
    }
    if let Some(softness) = columns.optional_real(&row, "shadow_softness_px")? {
        layout.shadow.softness_px = softness as f32;
    }
    if let Some(opacity) = columns.optional_real(&row, "shadow_opacity")? {
        layout.shadow.opacity = opacity as f32;
    }
    Ok(panel_settings)
}

#[cfg(test)]
mod tests {
    use super::{CREATE_TABLE, Setting, flag, measure, read_panel_settings, write_setting};
    use crate::state::WorkspacesPlacement;

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
            (Setting::FloatOffset, measure(3.25)),
            (Setting::TintFactor, measure(0.75)),
            (Setting::DisconnectedGroups, flag(true)),
            (Setting::AutoCollapseGroups, flag(false)),
            (Setting::DoNotDisturb, flag(true)),
            (Setting::Sound, flag(false)),
            (Setting::SpatialLayout, flag(false)),
            (Setting::WorkspacesInSidebar, flag(false)),
            (Setting::CornerRadius, measure(6.5)),
            (Setting::DropShadow, flag(false)),
            (Setting::ShadowSoftness, measure(33.0)),
            (Setting::ShadowOpacity, measure(0.5)),
        ] {
            write_setting(&conn, setting, value).await.unwrap();
        }

        let panel_settings = read_panel_settings(&conn).await.unwrap();
        let shadow = panel_settings.layout.shadow;
        assert!((panel_settings.layout.corner_radius.top_left - 6.5).abs() < f32::EPSILON);
        assert!(!shadow.cast);
        assert!((shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert!((panel_settings.layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!((panel_settings.layout.tint_factor - 0.75).abs() < f32::EPSILON);
        assert!(panel_settings.layout.disconnected_groups);
        assert!(!panel_settings.layout.auto_collapse_groups);
        assert!(panel_settings.toasts.do_not_disturb);
        assert!(!panel_settings.toasts.sound);
        assert!(!panel_settings.toasts.spatial_layout);
        assert_eq!(
            panel_settings.layout.workspaces_placement,
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = connection().await;
        write_setting(&conn, Setting::DoNotDisturb, flag(true))
            .await
            .unwrap();

        let panel_settings = read_panel_settings(&conn).await.unwrap();
        assert!(panel_settings.toasts.do_not_disturb);
        assert!(panel_settings.layout.auto_collapse_groups);
        assert!(panel_settings.toasts.sound);
        assert!(panel_settings.toasts.spatial_layout);
        assert_eq!(
            panel_settings.layout.workspaces_placement,
            WorkspacesPlacement::Sidebar
        );
    }
}
