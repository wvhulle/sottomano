use sottomano_sources::sqlite_state_store::{
    Columns, StoreError, flag, real, upsert_column, with_table,
};

use crate::{ShellLayer, ShellSettings, WorkspacesPlacement, layer_appearance};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS shell_settings (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    float_offset_px REAL,
    disconnected_groups INTEGER,
    auto_collapse_groups INTEGER,
    workspaces_in_sidebar INTEGER,
    sound INTEGER,
    toast_spatial_layout INTEGER
)";

const SELECT_SHELL_SETTINGS: &str = "SELECT float_offset_px, disconnected_groups,
        auto_collapse_groups, workspaces_in_sidebar, sound, toast_spatial_layout
    FROM shell_settings WHERE id = 0";

#[derive(Debug, Clone, Copy)]
pub enum ShellSetting {
    FloatOffset(f32),
    DisconnectedGroups(bool),
    AutoCollapseGroups(bool),
    Sound(bool),
    ToastSpatialLayout(bool),
    WorkspacesPlacement(WorkspacesPlacement),
}

impl ShellSetting {
    fn column(self) -> &'static str {
        match self {
            Self::FloatOffset(_) => "float_offset_px",
            Self::DisconnectedGroups(_) => "disconnected_groups",
            Self::AutoCollapseGroups(_) => "auto_collapse_groups",
            Self::Sound(_) => "sound",
            Self::ToastSpatialLayout(_) => "toast_spatial_layout",
            Self::WorkspacesPlacement(_) => "workspaces_in_sidebar",
        }
    }

    fn stored_value(self) -> turso::Value {
        match self {
            Self::FloatOffset(offset) => real(offset),
            Self::DisconnectedGroups(enabled)
            | Self::AutoCollapseGroups(enabled)
            | Self::Sound(enabled)
            | Self::ToastSpatialLayout(enabled) => flag(enabled),
            Self::WorkspacesPlacement(placement) => flag(placement == WorkspacesPlacement::Sidebar),
        }
    }
}

pub async fn load() -> ShellSettings {
    let shell_settings = with_table(
        "shell settings: load failed",
        &[CREATE_TABLE],
        read_shell_settings,
    )
    .await
    .unwrap_or_default();
    ShellSettings {
        panels: layer_appearance::load(ShellLayer::Panels, shell_settings.panels).await,
        sidebar: layer_appearance::load(ShellLayer::Sidebar, shell_settings.sidebar).await,
        overlays: layer_appearance::load(ShellLayer::Overlays, shell_settings.overlays).await,
        ..shell_settings
    }
}

pub async fn save(setting: ShellSetting) -> bool {
    with_table(
        &format!("shell settings: save {} failed", setting.column()),
        &[CREATE_TABLE],
        async |conn| write_setting(conn, setting).await,
    )
    .await
    .is_some()
}

async fn write_setting(conn: &turso::Connection, setting: ShellSetting) -> Result<(), StoreError> {
    upsert_column(
        conn,
        "shell_settings",
        setting.column(),
        setting.stored_value(),
    )
    .await
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
    if let Some(sound) = columns.optional_flag(&row, "sound") {
        notifications.sound = sound;
    }
    if let Some(spatial) = columns.optional_flag(&row, "toast_spatial_layout") {
        notifications.toast_spatial_layout = spatial;
    }
    Ok(shell_settings)
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, ShellSetting, read_shell_settings, write_setting};
    use crate::WorkspacesPlacement;

    #[tokio::test]
    async fn every_setting_round_trips_to_its_own_column() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        for setting in [
            ShellSetting::FloatOffset(3.25),
            ShellSetting::DisconnectedGroups(true),
            ShellSetting::AutoCollapseGroups(false),
            ShellSetting::Sound(false),
            ShellSetting::ToastSpatialLayout(false),
            ShellSetting::WorkspacesPlacement(WorkspacesPlacement::BottomPanel),
        ] {
            write_setting(&conn, setting).await.unwrap();
        }

        let shell_settings = read_shell_settings(&conn).await.unwrap();
        let layout = &shell_settings.layout;
        assert!((layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!(layout.disconnected_groups);
        assert!(!layout.auto_collapse_groups);
        assert!(!shell_settings.notifications.sound);
        assert!(!shell_settings.notifications.toast_spatial_layout);
        assert_eq!(
            layout.workspaces_placement,
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&conn, ShellSetting::Sound(false))
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
