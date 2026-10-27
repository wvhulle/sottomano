use sottomano_sources::sqlite_state_store::{
    Columns, StoreError, flag, real, upsert_column, with_table,
};

use crate::widget_groups::sidebar_settings_group::WorkspacesPlacement;

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

pub async fn save(setting: ShellSetting) -> bool {
    with_table(
        &format!("shell settings: save {} failed", setting.column()),
        &[CREATE_TABLE],
        async |connection| write_setting(connection, setting).await,
    )
    .await
    .is_some()
}

pub async fn load_stored_row<Stored: Copy>(
    failure_label: &str,
    fallback: Stored,
    extract: impl FnOnce(&Columns, &turso::Row, Stored) -> Stored,
) -> Stored {
    with_table(failure_label, &[CREATE_TABLE], async |connection| {
        read_stored_row(connection, fallback, extract).await
    })
    .await
    .unwrap_or(fallback)
}

pub async fn load_stored_flag(failure_label: &str, column: &str, fallback: bool) -> bool {
    load_stored_row(failure_label, fallback, |columns, row, fallback| {
        columns.optional_flag(row, column).unwrap_or(fallback)
    })
    .await
}

pub async fn write_setting(
    connection: &turso::Connection,
    setting: ShellSetting,
) -> Result<(), StoreError> {
    upsert_column(
        connection,
        "shell_settings",
        setting.column(),
        setting.stored_value(),
    )
    .await
}

pub async fn read_stored_row<Stored>(
    connection: &turso::Connection,
    fallback: Stored,
    extract: impl FnOnce(&Columns, &turso::Row, Stored) -> Stored,
) -> Result<Stored, StoreError> {
    let mut rows = connection.query(SELECT_SHELL_SETTINGS, ()).await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(fallback);
    };
    Ok(extract(&columns, &row, fallback))
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, ShellSetting, read_stored_row, write_setting};

    async fn stored_flag(connection: &turso::Connection, column: &str, fallback: bool) -> bool {
        read_stored_row(connection, fallback, |columns, row, fallback| {
            columns.optional_flag(row, column).unwrap_or(fallback)
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn notification_flags_round_trip_to_their_own_columns() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&connection, ShellSetting::Sound(false))
            .await
            .unwrap();
        write_setting(&connection, ShellSetting::ToastSpatialLayout(false))
            .await
            .unwrap();

        assert!(!stored_flag(&connection, "sound", true).await);
        assert!(!stored_flag(&connection, "toast_spatial_layout", true).await);
    }

    #[tokio::test]
    async fn unwritten_columns_keep_their_fallback() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_setting(&connection, ShellSetting::Sound(false))
            .await
            .unwrap();

        assert!(stored_flag(&connection, "toast_spatial_layout", true).await);
        assert!(stored_flag(&connection, "disconnected_groups", true).await);
    }
}
