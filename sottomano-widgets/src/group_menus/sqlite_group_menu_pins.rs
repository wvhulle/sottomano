use std::collections::BTreeSet;

use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

pub const CREATE_GROUP_PINS_TABLE: &str = "CREATE TABLE IF NOT EXISTS pinned_widget_groups (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, group_instance)
)";

pub const CREATE_ITEM_PINS_TABLE: &str = "CREATE TABLE IF NOT EXISTS pinned_group_items (
    menu_topic TEXT NOT NULL,
    group_family TEXT NOT NULL,
    group_instance TEXT NOT NULL,
    item_key TEXT NOT NULL,
    PRIMARY KEY (menu_topic, group_family, group_instance, item_key)
)";

const CREATE_PIN_TABLES: [&str; 2] = [CREATE_GROUP_PINS_TABLE, CREATE_ITEM_PINS_TABLE];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoredGroupMenuPin {
    Group {
        menu_topic: String,
        group_family: String,
        group_instance: String,
    },
    Item {
        menu_topic: String,
        group_family: String,
        group_instance: String,
        item_key: String,
    },
}

pub async fn load() -> BTreeSet<StoredGroupMenuPin> {
    with_table(
        "group menu pins: load failed",
        &CREATE_PIN_TABLES,
        read_pins,
    )
    .await
    .unwrap_or_default()
}

pub async fn save(pin: StoredGroupMenuPin, pinned: bool) -> bool {
    with_table(
        "group menu pins: save failed",
        &CREATE_PIN_TABLES,
        async |connection| write_pin(connection, &pin, pinned).await,
    )
    .await
    .is_some()
}

async fn read_pins(
    connection: &turso::Connection,
) -> Result<BTreeSet<StoredGroupMenuPin>, StoreError> {
    let mut pins = BTreeSet::new();
    let mut group_rows = connection
        .query(
            "SELECT menu_topic, group_family, group_instance FROM pinned_widget_groups",
            (),
        )
        .await?;
    let group_columns = Columns::of(&group_rows);
    while let Some(row) = group_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Group {
            menu_topic: group_columns.text(&row, "menu_topic")?,
            group_family: group_columns.text(&row, "group_family")?,
            group_instance: group_columns.text(&row, "group_instance")?,
        });
    }
    let mut item_rows = connection
        .query(
            "SELECT menu_topic, group_family, group_instance, item_key FROM pinned_group_items",
            (),
        )
        .await?;
    let item_columns = Columns::of(&item_rows);
    while let Some(row) = item_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Item {
            menu_topic: item_columns.text(&row, "menu_topic")?,
            group_family: item_columns.text(&row, "group_family")?,
            group_instance: item_columns.text(&row, "group_instance")?,
            item_key: item_columns.text(&row, "item_key")?,
        });
    }
    Ok(pins)
}

async fn write_pin(
    connection: &turso::Connection,
    pin: &StoredGroupMenuPin,
    pinned: bool,
) -> Result<(), StoreError> {
    match pin {
        StoredGroupMenuPin::Group {
            menu_topic,
            group_family,
            group_instance,
        } => {
            let statement = if pinned {
                "INSERT INTO pinned_widget_groups (menu_topic, group_family, group_instance) \
                 VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM pinned_widget_groups WHERE menu_topic = ?1 AND group_family = ?2 AND \
                 group_instance = ?3"
            };
            connection
                .execute(
                    statement,
                    (
                        menu_topic.as_str(),
                        group_family.as_str(),
                        group_instance.as_str(),
                    ),
                )
                .await?;
        }
        StoredGroupMenuPin::Item {
            menu_topic,
            group_family,
            group_instance,
            item_key,
        } => {
            let statement = if pinned {
                "INSERT INTO pinned_group_items (menu_topic, group_family, group_instance, \
                 item_key) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM pinned_group_items WHERE menu_topic = ?1 AND group_family = ?2 AND \
                 group_instance = ?3 AND item_key = ?4"
            };
            connection
                .execute(
                    statement,
                    (
                        menu_topic.as_str(),
                        group_family.as_str(),
                        group_instance.as_str(),
                        item_key.as_str(),
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_PIN_TABLES, StoredGroupMenuPin, read_pins, write_pin};

    fn gpu_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Group {
            menu_topic: "hardware".to_owned(),
            group_family: "gpu".to_owned(),
            group_instance: String::new(),
        }
    }

    fn gpu_load_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Item {
            menu_topic: "hardware".to_owned(),
            group_family: "gpu".to_owned(),
            group_instance: String::new(),
            item_key: "gpu_load".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_pin_survives_a_round_trip() {
        let connection = in_memory_connection(&CREATE_PIN_TABLES).await;
        write_pin(&connection, &gpu_in_hardware(), true)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), true)
            .await
            .unwrap();
        let pins = read_pins(&connection).await.unwrap();
        assert_eq!(pins.into_iter().collect::<Vec<_>>(), [gpu_in_hardware()]);
    }

    #[tokio::test]
    async fn unpinning_removes_the_stored_pin() {
        let connection = in_memory_connection(&CREATE_PIN_TABLES).await;
        write_pin(&connection, &gpu_in_hardware(), true)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), false)
            .await
            .unwrap();
        assert!(read_pins(&connection).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn item_and_group_pins_of_one_group_coexist() {
        let connection = in_memory_connection(&CREATE_PIN_TABLES).await;
        write_pin(&connection, &gpu_load_in_hardware(), true)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), true)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), false)
            .await
            .unwrap();
        let pins = read_pins(&connection).await.unwrap();
        assert_eq!(
            pins.into_iter().collect::<Vec<_>>(),
            [gpu_load_in_hardware()]
        );
    }
}
