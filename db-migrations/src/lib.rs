#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("database error: {0}")]
    Database(#[from] turso::Error),
    #[error("PRAGMA user_version returned no row")]
    MissingSchemaVersion,
    #[error("PRAGMA user_version is not an integer")]
    NonIntegerSchemaVersion,
    #[error("the store is at schema version {found}, newer than the latest known version {known}")]
    NewerSchema { found: i64, known: i64 },
}

pub struct SchemaMigration {
    pub version: i64,
    pub file_name: &'static str,
    pub statements: &'static str,
}

macro_rules! schema_migration {
    ($version:literal, $file_name:literal) => {
        SchemaMigration {
            version: $version,
            file_name: $file_name,
            statements: include_str!(concat!("../sql/", $file_name)),
        }
    };
}

pub const SCHEMA_MIGRATIONS: [SchemaMigration; 15] = [
    schema_migration!(1, "0001_baseline_tables.sql"),
    schema_migration!(2, "0002_guest_pins_to_virtualization_menu.sql"),
    schema_migration!(3, "0003_default_idle_timeouts.sql"),
    schema_migration!(4, "0004_focus_facet_values_as_bytes.sql"),
    schema_migration!(5, "0005_submenu_connector_choice.sql"),
    schema_migration!(6, "0006_leftover_focus_facet_text_to_bytes.sql"),
    schema_migration!(7, "0007_window_titles_capped.sql"),
    schema_migration!(8, "0008_window_menu_bar_inline.sql"),
    schema_migration!(9, "0009_idle_timeouts_relative.sql"),
    schema_migration!(10, "0010_idle_chain_group.sql"),
    schema_migration!(11, "0011_power_indicator_renames.sql"),
    schema_migration!(12, "0012_split_sole_and_instance_pins.sql"),
    schema_migration!(13, "0013_default_panel_pins.sql"),
    schema_migration!(14, "0014_clock_pin_moves_to_config.sql"),
    schema_migration!(15, "0015_steam_games_become_groups.sql"),
];

pub async fn apply_pending_migrations(
    connection: &turso::Connection,
) -> Result<(), MigrationError> {
    let applied_version = applied_schema_version(connection).await?;
    let known_version = latest_known_version();
    if applied_version > known_version {
        return Err(MigrationError::NewerSchema {
            found: applied_version,
            known: known_version,
        });
    }
    if applied_version == known_version {
        return Ok(());
    }
    for migration in &SCHEMA_MIGRATIONS {
        apply_migration_once(connection, migration).await?;
    }
    Ok(())
}

fn latest_known_version() -> i64 {
    SCHEMA_MIGRATIONS
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or_default()
}

pub async fn applied_schema_version(connection: &turso::Connection) -> Result<i64, MigrationError> {
    let mut rows = connection.query("PRAGMA user_version", ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or(MigrationError::MissingSchemaVersion)?;
    match row.get_value(0)? {
        turso::Value::Integer(schema_version) => Ok(schema_version),
        _ => Err(MigrationError::NonIntegerSchemaVersion),
    }
}

async fn apply_migration_once(
    connection: &turso::Connection,
    migration: &SchemaMigration,
) -> Result<(), MigrationError> {
    connection.execute("BEGIN IMMEDIATE", ()).await?;
    let migration_outcome = run_inside_transaction(connection, migration).await;
    match migration_outcome {
        Ok(()) => {
            connection.execute("COMMIT", ()).await?;
            Ok(())
        }
        Err(migration_error) => {
            if let Err(rollback_error) = connection.execute("ROLLBACK", ()).await {
                tracing::warn!(
                    "schema migration {}: rollback failed: {rollback_error}",
                    migration.file_name
                );
            }
            Err(migration_error)
        }
    }
}

async fn run_inside_transaction(
    connection: &turso::Connection,
    migration: &SchemaMigration,
) -> Result<(), MigrationError> {
    if applied_schema_version(connection).await? >= migration.version {
        return Ok(());
    }
    connection.execute_batch(migration.statements).await?;
    connection
        .execute(format!("PRAGMA user_version = {}", migration.version), ())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{
        MigrationError, SCHEMA_MIGRATIONS, applied_schema_version, apply_migration_once,
        apply_pending_migrations,
    };

    async fn empty_connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        database.connect().unwrap()
    }

    #[test]
    fn every_migration_file_is_registered_in_version_order() {
        let migrations_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("sql");
        let mut file_names: Vec<String> = fs::read_dir(migrations_directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        file_names.sort();
        let registered_names: Vec<&str> = SCHEMA_MIGRATIONS
            .iter()
            .map(|migration| migration.file_name)
            .collect();
        assert_eq!(file_names, registered_names);
        for (position, migration) in SCHEMA_MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version, i64::try_from(position).unwrap() + 1);
            assert!(
                migration
                    .file_name
                    .starts_with(&format!("{:04}_", migration.version))
            );
        }
    }

    #[tokio::test]
    async fn applying_twice_ends_at_the_latest_version() {
        let connection = empty_connection().await;
        apply_pending_migrations(&connection).await.unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        assert_eq!(
            applied_schema_version(&connection).await.unwrap(),
            i64::try_from(SCHEMA_MIGRATIONS.len()).unwrap()
        );
    }

    #[tokio::test]
    async fn a_store_from_a_newer_schema_is_refused() {
        let connection = empty_connection().await;
        connection
            .execute("PRAGMA user_version = 58", ())
            .await
            .unwrap();
        let refusal = apply_pending_migrations(&connection).await.unwrap_err();
        assert!(matches!(
            refusal,
            MigrationError::NewerSchema { found: 58, known } if known == i64::try_from(SCHEMA_MIGRATIONS.len()).unwrap()
        ));
    }

    #[tokio::test]
    async fn the_baseline_accepts_a_store_created_before_migrations_existed() {
        let connection = empty_connection().await;
        connection
            .execute(
                "CREATE TABLE pinned_widget_groups (
                    menu_topic TEXT NOT NULL,
                    group_family TEXT NOT NULL,
                    group_instance TEXT NOT NULL,
                    PRIMARY KEY (menu_topic, group_family, group_instance)
                )",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO pinned_widget_groups VALUES ('services', 'virtual_machines', '')",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        assert_eq!(
            applied_schema_version(&connection).await.unwrap(),
            i64::try_from(SCHEMA_MIGRATIONS.len()).unwrap()
        );
        let mut rows = connection
            .query(
                "SELECT menu_topic FROM pinned_sole_groups WHERE group_family = 'virtual_machines'",
                (),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(
            row.get_value(0).unwrap(),
            turso::Value::Text("virtualization".to_owned())
        );
    }

    #[tokio::test]
    async fn focus_facet_text_values_become_bytes() {
        let connection = empty_connection().await;
        for migration in SCHEMA_MIGRATIONS
            .iter()
            .filter(|migration| migration.version < 4)
        {
            apply_migration_once(&connection, migration).await.unwrap();
        }
        connection
            .execute(
                "INSERT INTO window_focus_spans (started_at, window_id, ended_at) VALUES \
                 ('2026-11-13T09:00:00Z', 7, '2026-11-13T09:05:00Z')",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO window_focus_span_facets (span_started_at, facet, value) VALUES \
                 ('2026-11-13T09:00:00Z', 'project', '/code/sottomano')",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        let mut rows = connection
            .query("SELECT value FROM window_focus_span_facets", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(
            row.get_value(0).unwrap(),
            turso::Value::Blob(b"/code/sottomano".to_vec())
        );
    }

    #[tokio::test]
    async fn leftover_focus_facet_text_values_become_bytes() {
        let connection = empty_connection().await;
        for migration in SCHEMA_MIGRATIONS
            .iter()
            .filter(|migration| migration.version < 6)
        {
            apply_migration_once(&connection, migration).await.unwrap();
        }
        connection
            .execute(
                "INSERT INTO window_focus_spans (started_at, window_id, ended_at) VALUES \
                 ('2026-11-13T10:19:26Z', 7, '2026-11-13T10:20:00Z')",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO window_focus_span_facets (span_started_at, facet, value) VALUES \
                 ('2026-11-13T10:19:26Z', 'app', 'kitty')",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        let mut rows = connection
            .query("SELECT value FROM window_focus_span_facets", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(
            row.get_value(0).unwrap(),
            turso::Value::Blob(b"kitty".to_vec())
        );
    }

    #[tokio::test]
    async fn absolute_idle_timeouts_become_offsets_from_the_previous_stage() {
        let connection = empty_connection().await;
        for migration in SCHEMA_MIGRATIONS
            .iter()
            .filter(|migration| migration.version < 9)
        {
            apply_migration_once(&connection, migration).await.unwrap();
        }
        connection
            .execute("DELETE FROM idle_action_timeouts", ())
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO idle_action_timeouts (idle_action, after_seconds) VALUES \
                 ('lock_user', 600), ('screen_off', 300), ('sleep', 1800), ('power_down', 7200)",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        let mut rows = connection
            .query(
                "SELECT idle_action, after_seconds FROM idle_action_offsets ORDER BY idle_action",
                (),
            )
            .await
            .unwrap();
        let mut offsets = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            offsets.push((row.get_value(0).unwrap(), row.get_value(1).unwrap()));
        }
        let text_and_seconds = |name: &str, seconds: i64| {
            (
                turso::Value::Text(name.to_owned()),
                turso::Value::Integer(seconds),
            )
        };
        assert_eq!(
            offsets,
            [
                text_and_seconds("lock_user", 600),
                text_and_seconds("power_down", 5400),
                text_and_seconds("screen_off", 0),
                text_and_seconds("sleep", 1200),
            ]
        );
    }

    #[tokio::test]
    async fn pins_of_the_five_idle_groups_become_one_pin_of_the_idle_chain() {
        let connection = empty_connection().await;
        for migration in SCHEMA_MIGRATIONS
            .iter()
            .filter(|migration| migration.version < 10)
        {
            apply_migration_once(&connection, migration).await.unwrap();
        }
        connection
            .execute(
                "INSERT INTO pinned_widget_groups (menu_topic, group_family, group_instance) \
                 VALUES ('session', 'idle_timeouts', 'lock'), ('session', 'idle_timeouts', \
                 'sleep'), ('session', 'media', 'lock')",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO pinned_group_items (menu_topic, group_family, group_instance, \
                 item_key) VALUES ('session', 'idle_timeouts', 'sleep', 'After screen off')",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        let mut rows = connection
            .query(
                "SELECT group_family, group_instance FROM pinned_instance_groups ORDER BY \
                 group_family",
                (),
            )
            .await
            .unwrap();
        let mut pinned_groups = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            pinned_groups.push((row.get_value(0).unwrap(), row.get_value(1).unwrap()));
        }
        let family_and_instance = |family: &str, instance: &str| {
            (
                turso::Value::Text(family.to_owned()),
                turso::Value::Text(instance.to_owned()),
            )
        };
        assert_eq!(
            pinned_groups,
            [
                family_and_instance("idle_timeouts", "chain"),
                family_and_instance("media", "lock"),
            ]
        );
        let mut item_rows = connection
            .query(
                "SELECT (SELECT COUNT(*) FROM pinned_sole_group_items) + (SELECT COUNT(*) FROM \
                 pinned_instance_group_items)",
                (),
            )
            .await
            .unwrap();
        let item_count = item_rows
            .next()
            .await
            .unwrap()
            .unwrap()
            .get_value(0)
            .unwrap();
        assert_eq!(item_count, turso::Value::Integer(0));
    }

    #[tokio::test]
    async fn sole_and_named_instance_pins_move_to_their_own_tables() {
        let connection = empty_connection().await;
        for migration in SCHEMA_MIGRATIONS
            .iter()
            .filter(|migration| migration.version < 12)
        {
            apply_migration_once(&connection, migration).await.unwrap();
        }
        connection
            .execute(
                "INSERT INTO pinned_widget_groups (menu_topic, group_family, group_instance) \
                 VALUES ('hardware', 'gpu', ''), ('media', 'media', 'Firefox')",
                (),
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO pinned_group_items (menu_topic, group_family, group_instance, \
                 item_key) VALUES ('hardware', 'gpu', '', 'Load'), ('media', 'media', 'Firefox', \
                 'Track')",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        let mut rows = connection
            .query(
                "SELECT 'sole group ' || group_family FROM pinned_sole_groups WHERE menu_topic = \
                 'hardware' UNION ALL SELECT 'instance group ' || group_instance FROM \
                 pinned_instance_groups UNION ALL SELECT 'sole item ' || item_key FROM \
                 pinned_sole_group_items UNION ALL SELECT 'instance item ' || group_instance || ' \
                 ' || item_key FROM pinned_instance_group_items",
                (),
            )
            .await
            .unwrap();
        let mut moved_pins = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            moved_pins.push(row.get::<String>(0).unwrap());
        }
        assert_eq!(
            moved_pins,
            [
                "sole group gpu",
                "instance group Firefox",
                "sole item Load",
                "instance item Firefox Track"
            ]
        );
    }

    async fn pinned_group_keys(connection: &turso::Connection) -> Vec<String> {
        let mut rows = connection
            .query(
                "SELECT menu_topic || '/' || group_family FROM pinned_sole_groups ORDER BY \
                 menu_topic, group_family",
                (),
            )
            .await
            .unwrap();
        let mut pinned_keys = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            pinned_keys.push(row.get::<String>(0).unwrap());
        }
        pinned_keys
    }

    #[tokio::test]
    async fn notifications_and_tray_start_pinned_and_stay_unpinned_once_removed() {
        let connection = empty_connection().await;
        apply_pending_migrations(&connection).await.unwrap();
        assert_eq!(
            pinned_group_keys(&connection).await,
            ["agenda/notification", "session/tray"]
        );
        connection
            .execute(
                "DELETE FROM pinned_sole_groups WHERE group_family = 'tray'",
                (),
            )
            .await
            .unwrap();
        apply_pending_migrations(&connection).await.unwrap();
        assert_eq!(
            pinned_group_keys(&connection).await,
            ["agenda/notification"]
        );
    }
}
