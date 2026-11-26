use std::collections::BTreeSet;

use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, in_state_store, with_state_store,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinChange {
    Pin,
    Unpin,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoredGroupInstance {
    Sole,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StoredGroupMenuPin {
    Group {
        menu_topic: String,
        group_family: String,
        group_instance: StoredGroupInstance,
    },
    Item {
        menu_topic: String,
        group_family: String,
        group_instance: StoredGroupInstance,
        indicator_name: String,
    },
}

pub async fn load() -> BTreeSet<StoredGroupMenuPin> {
    with_state_store("group menu pins: load failed", read_pins)
        .await
        .unwrap_or_default()
}

pub async fn save(pin: StoredGroupMenuPin, pin_change: PinChange) -> Result<(), StoreError> {
    in_state_store(async |connection| write_pin(connection, &pin, pin_change).await).await
}

async fn read_pins(
    connection: &turso::Connection,
) -> Result<BTreeSet<StoredGroupMenuPin>, StoreError> {
    let mut pins = BTreeSet::new();
    let mut sole_group_rows = connection
        .query(
            "SELECT menu_topic, group_family FROM pinned_sole_groups",
            (),
        )
        .await?;
    let sole_group_columns = Columns::from(&sole_group_rows);
    while let Some(row) = sole_group_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Group {
            menu_topic: sole_group_columns.text(&row, "menu_topic")?,
            group_family: sole_group_columns.text(&row, "group_family")?,
            group_instance: StoredGroupInstance::Sole,
        });
    }
    let mut instance_group_rows = connection
        .query(
            "SELECT menu_topic, group_family, group_instance FROM pinned_instance_groups",
            (),
        )
        .await?;
    let instance_group_columns = Columns::from(&instance_group_rows);
    while let Some(row) = instance_group_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Group {
            menu_topic: instance_group_columns.text(&row, "menu_topic")?,
            group_family: instance_group_columns.text(&row, "group_family")?,
            group_instance: StoredGroupInstance::Named(
                instance_group_columns.text(&row, "group_instance")?,
            ),
        });
    }
    let mut sole_item_rows = connection
        .query(
            "SELECT menu_topic, group_family, item_key FROM pinned_sole_group_items",
            (),
        )
        .await?;
    let sole_item_columns = Columns::from(&sole_item_rows);
    while let Some(row) = sole_item_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Item {
            menu_topic: sole_item_columns.text(&row, "menu_topic")?,
            group_family: sole_item_columns.text(&row, "group_family")?,
            group_instance: StoredGroupInstance::Sole,
            indicator_name: sole_item_columns.text(&row, "item_key")?,
        });
    }
    let mut instance_item_rows = connection
        .query(
            "SELECT menu_topic, group_family, group_instance, item_key FROM \
             pinned_instance_group_items",
            (),
        )
        .await?;
    let instance_item_columns = Columns::from(&instance_item_rows);
    while let Some(row) = instance_item_rows.next().await? {
        pins.insert(StoredGroupMenuPin::Item {
            menu_topic: instance_item_columns.text(&row, "menu_topic")?,
            group_family: instance_item_columns.text(&row, "group_family")?,
            group_instance: StoredGroupInstance::Named(
                instance_item_columns.text(&row, "group_instance")?,
            ),
            indicator_name: instance_item_columns.text(&row, "item_key")?,
        });
    }
    Ok(pins)
}

async fn write_pin(
    connection: &turso::Connection,
    pin: &StoredGroupMenuPin,
    pin_change: PinChange,
) -> Result<(), StoreError> {
    match pin {
        StoredGroupMenuPin::Group {
            menu_topic,
            group_family,
            group_instance,
        } => {
            write_group_pin(
                connection,
                menu_topic,
                group_family,
                group_instance,
                pin_change,
            )
            .await
        }
        StoredGroupMenuPin::Item {
            menu_topic,
            group_family,
            group_instance,
            indicator_name,
        } => {
            write_item_pin(
                connection,
                PinnedItem {
                    menu_topic,
                    group_family,
                    group_instance,
                    indicator_name,
                },
                pin_change,
            )
            .await
        }
    }
}

struct PinnedItem<'pin> {
    menu_topic: &'pin str,
    group_family: &'pin str,
    group_instance: &'pin StoredGroupInstance,
    indicator_name: &'pin str,
}

async fn write_group_pin(
    connection: &turso::Connection,
    menu_topic: &str,
    group_family: &str,
    group_instance: &StoredGroupInstance,
    pin_change: PinChange,
) -> Result<(), StoreError> {
    match group_instance {
        StoredGroupInstance::Sole => {
            let statement = match pin_change {
                PinChange::Pin => {
                    "INSERT INTO pinned_sole_groups (menu_topic, group_family) VALUES (?1, ?2) ON \
                     CONFLICT DO NOTHING"
                }
                PinChange::Unpin => {
                    "DELETE FROM pinned_sole_groups WHERE menu_topic = ?1 AND group_family = ?2"
                }
            };
            connection
                .execute(statement, (menu_topic, group_family))
                .await?;
        }
        StoredGroupInstance::Named(group_instance) => {
            let statement = match pin_change {
                PinChange::Pin => {
                    "INSERT INTO pinned_instance_groups (menu_topic, group_family, group_instance) \
                     VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING"
                }
                PinChange::Unpin => {
                    "DELETE FROM pinned_instance_groups WHERE menu_topic = ?1 AND group_family = \
                     ?2 AND group_instance = ?3"
                }
            };
            connection
                .execute(
                    statement,
                    (menu_topic, group_family, group_instance.as_str()),
                )
                .await?;
        }
    }
    Ok(())
}

async fn write_item_pin(
    connection: &turso::Connection,
    pinned_item: PinnedItem<'_>,
    pin_change: PinChange,
) -> Result<(), StoreError> {
    let PinnedItem {
        menu_topic,
        group_family,
        group_instance,
        indicator_name,
    } = pinned_item;
    match group_instance {
        StoredGroupInstance::Sole => {
            let statement = match pin_change {
                PinChange::Pin => {
                    "INSERT INTO pinned_sole_group_items (menu_topic, group_family, item_key) \
                     VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING"
                }
                PinChange::Unpin => {
                    "DELETE FROM pinned_sole_group_items WHERE menu_topic = ?1 AND group_family = \
                     ?2 AND item_key = ?3"
                }
            };
            connection
                .execute(statement, (menu_topic, group_family, indicator_name))
                .await?;
        }
        StoredGroupInstance::Named(group_instance) => {
            let statement = match pin_change {
                PinChange::Pin => {
                    "INSERT INTO pinned_instance_group_items (menu_topic, group_family, \
                     group_instance, item_key) VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING"
                }
                PinChange::Unpin => {
                    "DELETE FROM pinned_instance_group_items WHERE menu_topic = ?1 AND \
                     group_family = ?2 AND group_instance = ?3 AND item_key = ?4"
                }
            };
            connection
                .execute(
                    statement,
                    (
                        menu_topic,
                        group_family,
                        group_instance.as_str(),
                        indicator_name,
                    ),
                )
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{PinChange, StoredGroupInstance, StoredGroupMenuPin, read_pins, write_pin};

    fn gpu_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Group {
            menu_topic: "hardware".to_owned(),
            group_family: "gpu".to_owned(),
            group_instance: StoredGroupInstance::Sole,
        }
    }

    fn gpu_load_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Item {
            menu_topic: "hardware".to_owned(),
            group_family: "gpu".to_owned(),
            group_instance: StoredGroupInstance::Sole,
            indicator_name: "gpu_load".to_owned(),
        }
    }

    async fn pins_added_since(
        connection: &turso::Connection,
        migrated_pins: &BTreeSet<StoredGroupMenuPin>,
    ) -> Vec<StoredGroupMenuPin> {
        read_pins(connection)
            .await
            .unwrap()
            .difference(migrated_pins)
            .cloned()
            .collect()
    }

    #[tokio::test]
    async fn a_pin_survives_a_round_trip() {
        let connection = migrated_in_memory_store().await;
        let migrated_pins = read_pins(&connection).await.unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        assert_eq!(
            pins_added_since(&connection, &migrated_pins).await,
            [gpu_in_hardware()]
        );
    }

    #[tokio::test]
    async fn unpinning_removes_the_stored_pin() {
        let connection = migrated_in_memory_store().await;
        let migrated_pins = read_pins(&connection).await.unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Unpin)
            .await
            .unwrap();
        assert!(
            pins_added_since(&connection, &migrated_pins)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn item_and_group_pins_of_one_group_coexist() {
        let connection = migrated_in_memory_store().await;
        let migrated_pins = read_pins(&connection).await.unwrap();
        write_pin(&connection, &gpu_load_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Unpin)
            .await
            .unwrap();
        assert_eq!(
            pins_added_since(&connection, &migrated_pins).await,
            [gpu_load_in_hardware()]
        );
    }

    fn firefox_player_in_media() -> StoredGroupMenuPin {
        StoredGroupMenuPin::Group {
            menu_topic: "media".to_owned(),
            group_family: "media".to_owned(),
            group_instance: StoredGroupInstance::Named("Firefox".to_owned()),
        }
    }

    #[tokio::test]
    async fn sole_and_named_instance_pins_round_trip_apart() {
        let connection = migrated_in_memory_store().await;
        let migrated_pins = read_pins(&connection).await.unwrap();
        write_pin(&connection, &gpu_in_hardware(), PinChange::Pin)
            .await
            .unwrap();
        write_pin(&connection, &firefox_player_in_media(), PinChange::Pin)
            .await
            .unwrap();
        assert_eq!(
            pins_added_since(&connection, &migrated_pins).await,
            [gpu_in_hardware(), firefox_player_in_media()]
        );
    }
}
