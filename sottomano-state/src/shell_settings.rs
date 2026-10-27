use sottomano_sources::sqlite_state_store::{
    Columns, StoreError, flag, real, upsert_column, with_table,
};

use crate::WorkspacesPlacement;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelLayout {
    pub disconnected_groups: bool,
    pub auto_collapse_groups: bool,
    pub float_offset_px: f32,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            disconnected_groups: false,
            auto_collapse_groups: true,
            float_offset_px: 0.0,
        }
    }
}

pub async fn load_panel_layout() -> PanelLayout {
    with_table(
        "shell settings: load panel layout failed",
        &[CREATE_TABLE],
        read_panel_layout,
    )
    .await
    .unwrap_or_default()
}

pub async fn load_workspaces_placement() -> WorkspacesPlacement {
    with_table(
        "shell settings: load workspaces placement failed",
        &[CREATE_TABLE],
        read_workspaces_placement,
    )
    .await
    .unwrap_or(WorkspacesPlacement::Sidebar)
}

pub async fn load_alert_sound() -> bool {
    with_table(
        "shell settings: load sound failed",
        &[CREATE_TABLE],
        async |conn| read_stored_flag(conn, "sound", true).await,
    )
    .await
    .unwrap_or(true)
}

pub async fn load_toast_spatial_layout() -> bool {
    with_table(
        "shell settings: load toast spatial layout failed",
        &[CREATE_TABLE],
        async |conn| read_stored_flag(conn, "toast_spatial_layout", true).await,
    )
    .await
    .unwrap_or(true)
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

async fn read_stored_row<Stored>(
    conn: &turso::Connection,
    fallback: Stored,
    extract: impl FnOnce(&Columns, &turso::Row, Stored) -> Stored,
) -> Result<Stored, StoreError> {
    let mut rows = conn.query(SELECT_SHELL_SETTINGS, ()).await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(fallback);
    };
    Ok(extract(&columns, &row, fallback))
}

async fn read_panel_layout(conn: &turso::Connection) -> Result<PanelLayout, StoreError> {
    read_stored_row(conn, PanelLayout::default(), |columns, row, defaults| {
        PanelLayout {
            float_offset_px: columns
                .optional_real(row, "float_offset_px")
                .map_or(defaults.float_offset_px, |offset| offset as f32),
            disconnected_groups: columns
                .optional_flag(row, "disconnected_groups")
                .unwrap_or(defaults.disconnected_groups),
            auto_collapse_groups: columns
                .optional_flag(row, "auto_collapse_groups")
                .unwrap_or(defaults.auto_collapse_groups),
        }
    })
    .await
}

async fn read_workspaces_placement(
    conn: &turso::Connection,
) -> Result<WorkspacesPlacement, StoreError> {
    read_stored_row(
        conn,
        WorkspacesPlacement::Sidebar,
        |columns, row, fallback| match columns.optional_flag(row, "workspaces_in_sidebar") {
            Some(true) => WorkspacesPlacement::Sidebar,
            Some(false) => WorkspacesPlacement::BottomPanel,
            None => fallback,
        },
    )
    .await
}

async fn read_stored_flag(
    conn: &turso::Connection,
    column: &str,
    fallback: bool,
) -> Result<bool, StoreError> {
    read_stored_row(conn, fallback, |columns, row, fallback| {
        columns.optional_flag(row, column).unwrap_or(fallback)
    })
    .await
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{
        CREATE_TABLE, PanelLayout, ShellSetting, read_panel_layout, read_stored_flag,
        read_workspaces_placement, write_setting,
    };
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

        let panel_layout = read_panel_layout(&conn).await.unwrap();
        assert!((panel_layout.float_offset_px - 3.25).abs() < f32::EPSILON);
        assert!(panel_layout.disconnected_groups);
        assert!(!panel_layout.auto_collapse_groups);
        assert!(!read_stored_flag(&conn, "sound", true).await.unwrap());
        assert!(
            !read_stored_flag(&conn, "toast_spatial_layout", true)
                .await
                .unwrap()
        );
        assert_eq!(
            read_workspaces_placement(&conn).await.unwrap(),
            WorkspacesPlacement::BottomPanel
        );
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_defaults() {
        let conn = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&conn, ShellSetting::Sound(false))
            .await
            .unwrap();

        assert!(!read_stored_flag(&conn, "sound", true).await.unwrap());
        assert_eq!(
            read_panel_layout(&conn).await.unwrap(),
            PanelLayout::default()
        );
        assert_eq!(
            read_workspaces_placement(&conn).await.unwrap(),
            WorkspacesPlacement::Sidebar
        );
    }
}
