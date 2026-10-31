use std::collections::BTreeSet;

use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS group_menu_pins (
    group_menu TEXT NOT NULL,
    group_title TEXT NOT NULL,
    PRIMARY KEY (group_menu, group_title)
)";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StoredGroupMenuPin {
    pub group_menu: String,
    pub group_title: String,
}

pub async fn load() -> BTreeSet<StoredGroupMenuPin> {
    with_table("group menu pins: load failed", &[CREATE_TABLE], read_pins)
        .await
        .unwrap_or_default()
}

pub async fn save(pin: StoredGroupMenuPin, pinned: bool) -> bool {
    with_table(
        "group menu pins: save failed",
        &[CREATE_TABLE],
        async |connection| write_pin(connection, &pin, pinned).await,
    )
    .await
    .is_some()
}

async fn read_pins(
    connection: &turso::Connection,
) -> Result<BTreeSet<StoredGroupMenuPin>, StoreError> {
    let mut rows = connection
        .query("SELECT group_menu, group_title FROM group_menu_pins", ())
        .await?;
    let columns = Columns::of(&rows);
    let mut pins = BTreeSet::new();
    while let Some(row) = rows.next().await? {
        pins.insert(StoredGroupMenuPin {
            group_menu: columns.text(&row, "group_menu")?,
            group_title: columns.text(&row, "group_title")?,
        });
    }
    Ok(pins)
}

async fn write_pin(
    connection: &turso::Connection,
    pin: &StoredGroupMenuPin,
    pinned: bool,
) -> Result<(), StoreError> {
    let statement = if pinned {
        "INSERT INTO group_menu_pins (group_menu, group_title) VALUES (?1, ?2) ON CONFLICT DO \
         NOTHING"
    } else {
        "DELETE FROM group_menu_pins WHERE group_menu = ?1 AND group_title = ?2"
    };
    connection
        .execute(
            statement,
            (pin.group_menu.as_str(), pin.group_title.as_str()),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, StoredGroupMenuPin, read_pins, write_pin};

    fn gpu_in_hardware() -> StoredGroupMenuPin {
        StoredGroupMenuPin {
            group_menu: "HardwareMenu".to_owned(),
            group_title: "GPU".to_owned(),
        }
    }

    #[tokio::test]
    async fn a_pin_survives_a_round_trip() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
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
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        write_pin(&connection, &gpu_in_hardware(), true)
            .await
            .unwrap();
        write_pin(&connection, &gpu_in_hardware(), false)
            .await
            .unwrap();
        assert!(read_pins(&connection).await.unwrap().is_empty());
    }
}
