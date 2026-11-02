use std::collections::BTreeSet;

use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

pub const CREATE_GROUP_PINS_TABLE: &str = "CREATE TABLE IF NOT EXISTS group_menu_pins (
    group_menu TEXT NOT NULL,
    group_title TEXT NOT NULL,
    PRIMARY KEY (group_menu, group_title)
)";

pub const CREATE_ITEM_PINS_TABLE: &str = "CREATE TABLE IF NOT EXISTS group_menu_item_pins (
    group_menu TEXT NOT NULL,
    group_title TEXT NOT NULL,
    item_name TEXT NOT NULL,
    PRIMARY KEY (group_menu, group_title, item_name)
)";

const CREATE_PIN_TABLES: [&str; 2] = [CREATE_GROUP_PINS_TABLE, CREATE_ITEM_PINS_TABLE];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoredGroupMenuPin {
    Group {
        group_menu: String,
        group_title: String,
    },
    Item {
        group_menu: String,
        group_title: String,
        item_name: String,
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
        .query("SELECT group_menu, group_title FROM group_menu_pins", ())
        .await?;
    let group_columns = Columns::of(&group_rows);
    while let Some(row) = group_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Group {
            group_menu: group_columns.text(&row, "group_menu")?,
            group_title: group_columns.text(&row, "group_title")?,
        });
    }
    let mut item_rows = connection
        .query(
            "SELECT group_menu, group_title, item_name FROM group_menu_item_pins",
            (),
        )
        .await?;
    let item_columns = Columns::of(&item_rows);
    while let Some(row) = item_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Item {
            group_menu: item_columns.text(&row, "group_menu")?,
            group_title: item_columns.text(&row, "group_title")?,
            item_name: item_columns.text(&row, "item_name")?,
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
            group_menu,
            group_title,
        } => {
            let statement = if pinned {
                "INSERT INTO group_menu_pins (group_menu, group_title) VALUES (?1, ?2) ON CONFLICT \
                 DO NOTHING"
            } else {
                "DELETE FROM group_menu_pins WHERE group_menu = ?1 AND group_title = ?2"
            };
            connection
                .execute(statement, (group_menu.as_str(), group_title.as_str()))
                .await?;
        }
        StoredGroupMenuPin::Item {
            group_menu,
            group_title,
            item_name,
        } => {
            let statement = if pinned {
                "INSERT INTO group_menu_item_pins (group_menu, group_title, item_name) VALUES (?1, \
                 ?2, ?3) ON CONFLICT DO NOTHING"
            } else {
                "DELETE FROM group_menu_item_pins WHERE group_menu = ?1 AND group_title = ?2 AND \
                 item_name = ?3"
            };
            connection
                .execute(
                    statement,
                    (
                        group_menu.as_str(),
                        group_title.as_str(),
                        item_name.as_str(),
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
            group_menu: "HardwareMenu".to_owned(),
            group_title: "GPU".to_owned(),
        }
    }

    fn gpu_load_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Item {
            group_menu: "HardwareMenu".to_owned(),
            group_title: "GPU".to_owned(),
            item_name: "GPU load".to_owned(),
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
