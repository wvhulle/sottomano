use std::collections::BTreeSet;

use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, in_state_store, with_state_store,
};

use crate::group_menus::group_menu_state::MenuPinState;

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
        item_in_group_name: String,
    },
}

pub async fn load() -> BTreeSet<StoredGroupMenuPin> {
    with_state_store("group menu pins: load failed", read_pins)
        .await
        .unwrap_or_default()
}

pub async fn save(pin: StoredGroupMenuPin, pin_state: MenuPinState) -> Result<(), StoreError> {
    in_state_store(async |connection| write_pin(connection, &pin, pin_state).await).await
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
            item_in_group_name: item_columns.text(&row, "item_key")?,
        });
    }
    Ok(pins)
}

async fn write_pin(
    connection: &turso::Connection,
    pin: &StoredGroupMenuPin,
    pin_state: MenuPinState,
) -> Result<(), StoreError> {
    match pin {
        StoredGroupMenuPin::Group {
            menu_topic,
            group_family,
            group_instance,
        } => {
            let statement = match pin_state {
                MenuPinState::Pinned => {
                    "INSERT INTO pinned_widget_groups (menu_topic, group_family, group_instance) \
                     VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING"
                }
                MenuPinState::Unpinned => {
                    "DELETE FROM pinned_widget_groups WHERE menu_topic = ?1 AND group_family = ?2 \
                     AND group_instance = ?3"
                }
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
            item_in_group_name,
        } => {
            let statement = match pin_state {
                MenuPinState::Pinned => {
                    "INSERT INTO pinned_group_items (menu_topic, group_family, group_instance, \
                     item_key) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING"
                }
                MenuPinState::Unpinned => {
                    "DELETE FROM pinned_group_items WHERE menu_topic = ?1 AND group_family = ?2 \
                     AND group_instance = ?3 AND item_key = ?4"
                }
            };
            connection
                .execute(
                    statement,
                    (
                        menu_topic.as_str(),
                        group_family.as_str(),
                        group_instance.as_str(),
                        item_in_group_name.as_str(),
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{MenuPinState, StoredGroupMenuPin, read_pins, write_pin};

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
            item_in_group_name: "gpu_load".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_pin_survives_a_round_trip() {
        let connection = migrated_in_memory_store().await;
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Pinned)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Pinned)
            .await
            .unwrap();
        let pins = read_pins(&connection).await.unwrap();
        assert_eq!(pins.into_iter().collect::<Vec<_>>(), [gpu_in_hardware()]);
    }

    #[tokio::test]
    async fn unpinning_removes_the_stored_pin() {
        let connection = migrated_in_memory_store().await;
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Pinned)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Unpinned)
            .await
            .unwrap();
        assert!(read_pins(&connection).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn item_and_group_pins_of_one_group_coexist() {
        let connection = migrated_in_memory_store().await;
        write_pin(&connection, &gpu_load_in_hardware(), MenuPinState::Pinned)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Pinned)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), MenuPinState::Unpinned)
            .await
            .unwrap();
        let pins = read_pins(&connection).await.unwrap();
        assert_eq!(
            pins.into_iter().collect::<Vec<_>>(),
            [gpu_load_in_hardware()]
        );
    }
}
