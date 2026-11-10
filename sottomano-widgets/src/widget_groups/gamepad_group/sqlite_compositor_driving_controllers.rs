use std::collections::BTreeSet;

use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, in_state_store, with_state_store,
};

pub async fn load() -> BTreeSet<String> {
    with_state_store(
        "gamepad compositor control: load failed",
        read_driving_controllers,
    )
    .await
    .unwrap_or_default()
}

pub async fn save(driving_controllers: BTreeSet<String>) -> Result<(), StoreError> {
    in_state_store(async |connection| {
        write_driving_controllers(connection, &driving_controllers).await
    })
    .await
}

async fn read_driving_controllers(
    connection: &turso::Connection,
) -> Result<BTreeSet<String>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT controller_key FROM compositor_driving_controllers",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let mut driving_controllers = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        driving_controllers.insert(columns.text(&row, "controller_key")?);
    }
    Ok(driving_controllers)
}

async fn write_driving_controllers(
    connection: &turso::Connection,
    driving_controllers: &BTreeSet<String>,
) -> Result<(), StoreError> {
    connection
        .execute("DELETE FROM compositor_driving_controllers", ())
        .await?;
    for controller_key in driving_controllers {
        connection
            .execute(
                "INSERT INTO compositor_driving_controllers (controller_key) VALUES (?1)",
                (controller_key.as_str(),),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::*;

    #[tokio::test]
    async fn an_empty_table_hands_every_controller_to_the_focused_window() {
        let connection = migrated_in_memory_store().await;
        assert!(
            read_driving_controllers(&connection)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn saved_driving_controllers_replace_the_previous_ones() {
        let connection = migrated_in_memory_store().await;
        let first_save = BTreeSet::from(["AA:BB".to_owned(), "input31".to_owned()]);
        write_driving_controllers(&connection, &first_save)
            .await
            .unwrap();
        assert_eq!(
            read_driving_controllers(&connection).await.unwrap(),
            first_save
        );
        let second_save = BTreeSet::from(["AA:BB".to_owned()]);
        write_driving_controllers(&connection, &second_save)
            .await
            .unwrap();
        assert_eq!(
            read_driving_controllers(&connection).await.unwrap(),
            second_save
        );
    }
}
