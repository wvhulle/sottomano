use sottomano_sources::sqlite_state_store::{Columns, StoreError, flag, with_table};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingletonFlag {
    NotificationSound,
    ToastSpatialLayout,
    WorkspacesInSidebar,
    AutoCollapseWorkspaces,
    WindowsGroupedByLocation,
    FrequentAppsHideOpen,
    CategoryAppsSortedByUsage,
}

impl SingletonFlag {
    const fn create_table(self) -> &'static str {
        match self {
            Self::NotificationSound => {
                "CREATE TABLE IF NOT EXISTS notification_sound (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::ToastSpatialLayout => {
                "CREATE TABLE IF NOT EXISTS toast_spatial_layout (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::WorkspacesInSidebar => {
                "CREATE TABLE IF NOT EXISTS workspaces_in_sidebar (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::AutoCollapseWorkspaces => {
                "CREATE TABLE IF NOT EXISTS auto_collapse_workspaces (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::WindowsGroupedByLocation => {
                "CREATE TABLE IF NOT EXISTS windows_grouped_by_location (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::FrequentAppsHideOpen => {
                "CREATE TABLE IF NOT EXISTS frequent_apps_hide_open (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
            Self::CategoryAppsSortedByUsage => {
                "CREATE TABLE IF NOT EXISTS category_apps_sorted_by_usage (
                    id INTEGER PRIMARY KEY CHECK (id = 0),
                    enabled INTEGER NOT NULL
                )"
            }
        }
    }

    const fn table(self) -> &'static str {
        match self {
            Self::NotificationSound => "notification_sound",
            Self::ToastSpatialLayout => "toast_spatial_layout",
            Self::WorkspacesInSidebar => "workspaces_in_sidebar",
            Self::AutoCollapseWorkspaces => "auto_collapse_workspaces",
            Self::WindowsGroupedByLocation => "windows_grouped_by_location",
            Self::FrequentAppsHideOpen => "frequent_apps_hide_open",
            Self::CategoryAppsSortedByUsage => "category_apps_sorted_by_usage",
        }
    }
}

pub async fn load(stored_flag: SingletonFlag, fallback: bool) -> bool {
    with_table(
        &format!("{}: load failed", stored_flag.table()),
        &[stored_flag.create_table()],
        async |connection| read_flag(connection, stored_flag, fallback).await,
    )
    .await
    .unwrap_or(fallback)
}

pub async fn save(stored_flag: SingletonFlag, enabled: bool) -> bool {
    with_table(
        &format!("{}: save failed", stored_flag.table()),
        &[stored_flag.create_table()],
        async |connection| write_flag(connection, stored_flag, enabled).await,
    )
    .await
    .is_some()
}

async fn read_flag(
    connection: &turso::Connection,
    stored_flag: SingletonFlag,
    fallback: bool,
) -> Result<bool, StoreError> {
    let mut rows = connection
        .query(
            format!("SELECT enabled FROM {} WHERE id = 0", stored_flag.table()),
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    match rows.next().await? {
        Some(row) => Ok(columns.integer(&row, "enabled")? != 0),
        None => Ok(fallback),
    }
}

async fn write_flag(
    connection: &turso::Connection,
    stored_flag: SingletonFlag,
    enabled: bool,
) -> Result<(), StoreError> {
    connection
        .execute(
            format!(
                "INSERT INTO {} (id, enabled) VALUES (0, ?1)
                 ON CONFLICT(id) DO UPDATE SET enabled = ?1",
                stored_flag.table()
            ),
            (flag(enabled),),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{SingletonFlag, read_flag, write_flag};

    const ALL_FLAGS: [SingletonFlag; 5] = [
        SingletonFlag::NotificationSound,
        SingletonFlag::ToastSpatialLayout,
        SingletonFlag::WorkspacesInSidebar,
        SingletonFlag::AutoCollapseWorkspaces,
        SingletonFlag::WindowsGroupedByLocation,
    ];

    #[tokio::test]
    async fn an_unwritten_flag_keeps_its_fallback() {
        for stored_flag in ALL_FLAGS {
            let connection = in_memory_connection(&[stored_flag.create_table()]).await;
            assert!(read_flag(&connection, stored_flag, true).await.unwrap());
            assert!(!read_flag(&connection, stored_flag, false).await.unwrap());
        }
    }

    #[tokio::test]
    async fn every_flag_round_trips_in_its_own_table() {
        for stored_flag in ALL_FLAGS {
            let connection = in_memory_connection(&[stored_flag.create_table()]).await;
            write_flag(&connection, stored_flag, false).await.unwrap();
            assert!(!read_flag(&connection, stored_flag, true).await.unwrap());
            write_flag(&connection, stored_flag, true).await.unwrap();
            assert!(read_flag(&connection, stored_flag, false).await.unwrap());
        }
    }

    #[tokio::test]
    async fn a_flag_table_refuses_a_missing_value() {
        let stored_flag = SingletonFlag::NotificationSound;
        let connection = in_memory_connection(&[stored_flag.create_table()]).await;
        let refused = connection
            .execute(
                "INSERT INTO notification_sound (id, enabled) VALUES (0, NULL)",
                (),
            )
            .await;
        assert!(refused.is_err());
    }
}
