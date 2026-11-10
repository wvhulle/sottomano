use crate::sqlite_state_store::{ColumnError, StoreError};

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
            statements: include_str!(concat!("../../../db-migrations/", $file_name)),
        }
    };
}

pub const SCHEMA_MIGRATIONS: [SchemaMigration; 3] = [
    schema_migration!(1, "0001_baseline_tables.sql"),
    schema_migration!(2, "0002_guest_pins_to_virtualization_menu.sql"),
    schema_migration!(3, "0003_default_idle_timeouts.sql"),
];

pub async fn apply_pending_migrations(connection: &turso::Connection) -> Result<(), StoreError> {
    if applied_schema_version(connection).await? >= latest_known_version() {
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

pub async fn applied_schema_version(connection: &turso::Connection) -> Result<i64, StoreError> {
    let mut rows = connection.query("PRAGMA user_version", ()).await?;
    let row = rows.next().await?.ok_or(StoreError::NoRow)?;
    match row.get_value(0)? {
        turso::Value::Integer(schema_version) => Ok(schema_version),
        _ => Err(ColumnError::NotInteger {
            name: "user_version".to_owned(),
        }
        .into()),
    }
}

async fn apply_migration_once(
    connection: &turso::Connection,
    migration: &SchemaMigration,
) -> Result<(), StoreError> {
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
) -> Result<(), StoreError> {
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

    use super::{SCHEMA_MIGRATIONS, applied_schema_version, apply_pending_migrations};
    use crate::sqlite_state_store::Columns;

    async fn empty_connection() -> turso::Connection {
        let database = turso::Builder::new_local(":memory:").build().await.unwrap();
        database.connect().unwrap()
    }

    #[test]
    fn every_migration_file_is_registered_in_version_order() {
        let migrations_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../db-migrations");
        let mut file_names: Vec<String> = fs::read_dir(migrations_directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
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
            .query("SELECT menu_topic FROM pinned_widget_groups", ())
            .await
            .unwrap();
        let columns = Columns::of(&rows);
        let row = rows.next().await.unwrap().unwrap();
        assert_eq!(columns.text(&row, "menu_topic").unwrap(), "virtualization");
    }
}
