use crate::state::{
    Columns, ShellSettings, StoreError, WorkspacesPlacement, connect, overlay_settings,
};

pub(crate) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS shell_settings (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    float_offset_px REAL,
    disconnected_groups INTEGER,
    auto_collapse_groups INTEGER,
    workspaces_in_sidebar INTEGER,
    panel_opacity REAL,
    panel_corner_radius_px REAL,
    drop_shadow INTEGER,
    shadow_softness_px REAL,
    shadow_opacity REAL,
    do_not_disturb INTEGER,
    sound INTEGER
)";

const SELECT_SHELL_SETTINGS: &str = "SELECT float_offset_px, disconnected_groups,
        auto_collapse_groups, workspaces_in_sidebar, panel_opacity,
        panel_corner_radius_px, drop_shadow, shadow_softness_px, shadow_opacity,
        do_not_disturb, sound
    FROM shell_settings WHERE id = 0";

#[derive(Debug, Clone, Copy)]
enum Setting {
    FloatOffset,
    PanelOpacity,
    DisconnectedGroups,
    AutoCollapseGroups,
    DoNotDisturb,
    Sound,
    WorkspacesInSidebar,
    PanelCornerRadius,
    DropShadow,
    ShadowSoftness,
    ShadowOpacity,
}

impl Setting {
    fn column(self) -> &'static str {
        match self {
            Self::PanelCornerRadius => "panel_corner_radius_px",
            Self::DropShadow => "drop_shadow",
            Self::ShadowSoftness => "shadow_softness_px",
            Self::ShadowOpacity => "shadow_opacity",
            Self::FloatOffset => "float_offset_px",
            Self::PanelOpacity => "panel_opacity",
            Self::DisconnectedGroups => "disconnected_groups",
            Self::AutoCollapseGroups => "auto_collapse_groups",
            Self::DoNotDisturb => "do_not_disturb",
            Self::Sound => "sound",
            Self::WorkspacesInSidebar => "workspaces_in_sidebar",
        }
    }
}

pub async fn load() -> ShellSettings {
    let mut shell_settings = try_load().await.unwrap_or_else(|e| {
        tracing::warn!("shell settings: load failed: {e}");
        ShellSettings::default()
    });
    shell_settings.overlays = overlay_settings::load().await;
    shell_settings
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

pub async fn save_panel_opacity(opacity: f32) -> bool {
    put(Setting::PanelOpacity, measure(opacity)).await
}

pub async fn save_panel_corner_radius(radius: f32) -> bool {
    put(Setting::PanelCornerRadius, measure(radius)).await
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
            tracing::warn!("shell settings: save {} failed: {e}", setting.column());
            false
        }
    }
}

async fn try_put(setting: Setting, value: turso::Value) -> Result<(), StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
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
            "INSERT INTO shell_settings (id, {column}) VALUES (0, ?1)
             ON CONFLICT(id) DO UPDATE SET {column} = ?1"
        ),
        (value,),
    )
    .await?;
    Ok(())
}

async fn try_load() -> Result<ShellSettings, StoreError> {
    let conn = connect(&[CREATE_TABLE]).await?;
    read_shell_settings(&conn).await
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
    let notifications = &mut shell_settings.notifications;
    if let Some(dnd) = columns.optional_flag(&row, "do_not_disturb") {
        notifications.do_not_disturb = dnd;
    }
    if let Some(sound) = columns.optional_flag(&row, "sound") {
        notifications.sound = sound;
    }
    let panels = &mut shell_settings.panels;
    if let Some(opacity) = columns.optional_real(&row, "panel_opacity") {
        panels.opacity = opacity as f32;
    }
    if let Some(radius) = columns.optional_real(&row, "panel_corner_radius_px") {
        panels.corner_radius = (radius as f32).into();
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
    use super::{CREATE_TABLE, Setting, flag, measure, read_shell_settings, write_setting};
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
            (Setting::PanelOpacity, measure(0.75)),
            (Setting::DisconnectedGroups, flag(true)),
            (Setting::AutoCollapseGroups, flag(false)),
            (Setting::DoNotDisturb, flag(true)),
            (Setting::Sound, flag(false)),
            (Setting::WorkspacesInSidebar, flag(false)),
            (Setting::PanelCornerRadius, measure(6.5)),
            (Setting::DropShadow, flag(false)),
            (Setting::ShadowSoftness, measure(33.0)),
            (Setting::ShadowOpacity, measure(0.5)),
        ] {
            write_setting(&conn, setting, value).await.unwrap();
        }

        let shell_settings = read_shell_settings(&conn).await.unwrap();
        let panels = &shell_settings.panels;
        let layout = &shell_settings.layout;
        assert!((panels.corner_radius.top_left - 6.5).abs() < f32::EPSILON);
        let shadow = shell_settings.shadow;
        assert!(!shadow.cast);
        assert!((shadow.softness_px - 33.0).abs() < f32::EPSILON);
        assert!((shadow.opacity - 0.5).abs() < f32::EPSILON);
        assert!((layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!((panels.opacity - 0.75).abs() < f32::EPSILON);
        assert!(layout.disconnected_groups);
        assert!(!layout.auto_collapse_groups);
        assert!(shell_settings.notifications.do_not_disturb);
        assert!(!shell_settings.notifications.sound);
        assert_eq!(
            layout.workspaces_placement,
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = connection().await;
        write_setting(&conn, Setting::DoNotDisturb, flag(true))
            .await
            .unwrap();

        let shell_settings = read_shell_settings(&conn).await.unwrap();
        assert!(shell_settings.notifications.do_not_disturb);
        assert!(shell_settings.layout.auto_collapse_groups);
        assert!(shell_settings.notifications.sound);
        assert_eq!(
            shell_settings.layout.workspaces_placement,
            WorkspacesPlacement::Sidebar
        );
    }
}
