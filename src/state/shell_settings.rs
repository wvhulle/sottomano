use crate::state::{
    Columns, ShellSettings, StoreError, WorkspacesPlacement, flag, overlay_settings, real,
    upsert_column, with_table,
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS shell_settings (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    float_offset_px REAL,
    disconnected_groups INTEGER,
    auto_collapse_groups INTEGER,
    workspaces_in_sidebar INTEGER,
    panel_opacity REAL,
    panel_corner_radius_px REAL,
    surface_outline INTEGER,
    drop_shadow INTEGER,
    shadow_softness_px REAL,
    shadow_opacity REAL,
    sound INTEGER
)";

const SELECT_SHELL_SETTINGS: &str = "SELECT float_offset_px, disconnected_groups,
        auto_collapse_groups, workspaces_in_sidebar, panel_opacity,
        panel_corner_radius_px, surface_outline, drop_shadow, shadow_softness_px, shadow_opacity,
        sound
    FROM shell_settings WHERE id = 0";

#[derive(Debug, Clone, Copy)]
enum Setting {
    FloatOffset,
    PanelOpacity,
    DisconnectedGroups,
    AutoCollapseGroups,
    Sound,
    WorkspacesInSidebar,
    PanelCornerRadius,
    SurfaceOutline,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
}

impl Setting {
    fn column(self) -> &'static str {
        match self {
            Self::PanelCornerRadius => "panel_corner_radius_px",
            Self::SurfaceOutline => "surface_outline",
            Self::DropShadow => "drop_shadow",
            Self::ShadowSoftness => "shadow_softness_px",
            Self::ShadowOpacity => "shadow_opacity",
            Self::FloatOffset => "float_offset_px",
            Self::PanelOpacity => "panel_opacity",
            Self::DisconnectedGroups => "disconnected_groups",
            Self::AutoCollapseGroups => "auto_collapse_groups",
            Self::Sound => "sound",
            Self::WorkspacesInSidebar => "workspaces_in_sidebar",
        }
    }
}

pub async fn load() -> ShellSettings {
    let mut shell_settings = with_table(
        "shell settings: load failed",
        &[CREATE_TABLE],
        read_shell_settings,
    )
    .await
    .unwrap_or_default();
    shell_settings.overlays = overlay_settings::load().await;
    shell_settings
}

pub async fn save_float_offset(offset: f32) -> bool {
    put(Setting::FloatOffset, real(offset)).await
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

pub async fn save_panel_opacity(opacity: f32) -> bool {
    put(Setting::PanelOpacity, real(opacity)).await
}

pub async fn save_panel_corner_radius(radius: f32) -> bool {
    put(Setting::PanelCornerRadius, real(radius)).await
}

pub async fn save_surface_outline(drawn: bool) {
    put(Setting::SurfaceOutline, flag(drawn)).await;
}

pub async fn save_drop_shadow(cast: bool) {
    put(Setting::DropShadow, flag(cast)).await;
}

pub async fn save_shadow_softness(softness: f32) -> bool {
    put(Setting::ShadowSoftness, real(softness)).await
}

pub async fn save_shadow_opacity(opacity: f32) -> bool {
    put(Setting::ShadowOpacity, real(opacity)).await
}

pub async fn save_sound(sound: bool) {
    put(Setting::Sound, flag(sound)).await;
}

async fn put(setting: Setting, value: turso::Value) -> bool {
    with_table(
        &format!("shell settings: save {} failed", setting.column()),
        &[CREATE_TABLE],
        async |conn| write_setting(conn, setting, value).await,
    )
    .await
    .is_some()
}

async fn write_setting(
    conn: &turso::Connection,
    setting: Setting,
    value: turso::Value,
) -> Result<(), StoreError> {
    upsert_column(conn, "shell_settings", setting.column(), value).await
}

async fn read_shell_settings(conn: &turso::Connection) -> Result<ShellSettings, StoreError> {
    let mut rows = conn.query(SELECT_SHELL_SETTINGS, ()).await?;
    let columns = Columns::of(&rows);
    let mut shell_settings = ShellSettings::default();
    let Some(row) = rows.next().await? else {
        return Ok(shell_settings);
    };
    let layout = &mut shell_settings.layout;
    if let Some(offset) = columns.optional_real(&row, "float_offset_px") {
        layout.float_offset_px = offset as f32;
    }
    if let Some(disconnected) = columns.optional_flag(&row, "disconnected_groups") {
        layout.disconnected_groups = disconnected;
    }
    if let Some(collapse) = columns.optional_flag(&row, "auto_collapse_groups") {
        layout.auto_collapse_groups = collapse;
    }
    if let Some(in_sidebar) = columns.optional_flag(&row, "workspaces_in_sidebar") {
        layout.workspaces_placement = if in_sidebar {
            WorkspacesPlacement::Sidebar
        } else {
            WorkspacesPlacement::BottomPanel
        };
    }
    if let Some(sound) = columns.optional_flag(&row, "sound") {
        shell_settings.notifications.sound = sound;
    }
    let panels = &mut shell_settings.panels;
    if let Some(opacity) = columns.optional_real(&row, "panel_opacity") {
        panels.opacity = opacity as f32;
    }
    if let Some(radius) = columns.optional_real(&row, "panel_corner_radius_px") {
        panels.corner_radius = (radius as f32).into();
    }
    if let Some(drawn) = columns.optional_flag(&row, "surface_outline") {
        shell_settings.outline.drawn = drawn;
    }
    let shadow = &mut shell_settings.shadow;
    if let Some(cast) = columns.optional_flag(&row, "drop_shadow") {
        shadow.cast = cast;
    }
    if let Some(softness) = columns.optional_real(&row, "shadow_softness_px") {
        shadow.softness_px = softness as f32;
    }
    if let Some(opacity) = columns.optional_real(&row, "shadow_opacity") {
        shadow.opacity = opacity as f32;
    }
    Ok(shell_settings)
}

#[cfg(test)]
mod tests {
    use super::{CREATE_TABLE, Setting, flag, read_shell_settings, real, write_setting};
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
            (Setting::FloatOffset, real(3.25)),
            (Setting::PanelOpacity, real(0.75)),
            (Setting::DisconnectedGroups, flag(true)),
            (Setting::AutoCollapseGroups, flag(false)),
            (Setting::Sound, flag(false)),
            (Setting::WorkspacesInSidebar, flag(false)),
            (Setting::PanelCornerRadius, real(6.5)),
            (Setting::SurfaceOutline, flag(false)),
            (Setting::DropShadow, flag(false)),
            (Setting::ShadowSoftness, real(33.0)),
            (Setting::ShadowOpacity, real(0.5)),
        ] {
            write_setting(&conn, setting, value).await.unwrap();
        }

        let shell_settings = read_shell_settings(&conn).await.unwrap();
        let panels = &shell_settings.panels;
        let layout = &shell_settings.layout;
        assert!((panels.corner_radius.top_left - 6.5).abs() < f32::EPSILON);
        assert!(!shell_settings.outline.drawn);
        let shadow = shell_settings.shadow;
        assert!(!shadow.cast);
        assert!((shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert!((layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!((panels.opacity - 0.75).abs() < f32::EPSILON);
        assert!(layout.disconnected_groups);
        assert!(!layout.auto_collapse_groups);
        assert!(!shell_settings.notifications.sound);
        assert_eq!(
            layout.workspaces_placement,
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = connection().await;
        write_setting(&conn, Setting::Sound, flag(false))
            .await
            .unwrap();

        let shell_settings = read_shell_settings(&conn).await.unwrap();
        assert!(!shell_settings.notifications.sound);
        assert!(shell_settings.layout.auto_collapse_groups);
        assert_eq!(
            shell_settings.layout.workspaces_placement,
            WorkspacesPlacement::Sidebar
        );
    }
}
