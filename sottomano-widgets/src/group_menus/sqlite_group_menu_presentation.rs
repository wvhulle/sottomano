use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, in_state_store, with_state_store,
};

use crate::group_menus::group_menu_presentation::{
    GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, OpenGroupLimit,
};

const fn layout_name(layout: GroupMenuLayout) -> &'static str {
    match layout {
        GroupMenuLayout::Cascading => "cascading",
        GroupMenuLayout::Accordion => "accordion",
    }
}

const fn open_trigger_name(trigger: GroupMenuOpenTrigger) -> &'static str {
    match trigger {
        GroupMenuOpenTrigger::Hover => "hover",
        GroupMenuOpenTrigger::Press => "press",
    }
}

const fn open_limit_name(limit: OpenGroupLimit) -> &'static str {
    match limit {
        OpenGroupLimit::One => "one",
        OpenGroupLimit::Several => "several",
    }
}

pub async fn load() -> GroupMenuPresentation {
    with_state_store("group menu presentation: load failed", read_presentation)
        .await
        .unwrap_or_default()
}

pub async fn save(presentation: GroupMenuPresentation) -> Result<(), StoreError> {
    in_state_store(async |connection| write_presentation(connection, presentation).await).await
}

async fn read_presentation(
    connection: &turso::Connection,
) -> Result<GroupMenuPresentation, StoreError> {
    let mut rows = connection
        .query(
            "SELECT layout, cascading_reveal_trigger, accordion_open_limit
             FROM group_menu_opening_choices WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(GroupMenuPresentation::default());
    };
    Ok(GroupMenuPresentation {
        layout: match columns.text(&row, "layout")?.as_str() {
            "accordion" => GroupMenuLayout::Accordion,
            _ => GroupMenuLayout::Cascading,
        },
        open_trigger: match columns.text(&row, "cascading_reveal_trigger")?.as_str() {
            "press" => GroupMenuOpenTrigger::Press,
            _ => GroupMenuOpenTrigger::Hover,
        },
        accordion_open_limit: match columns.text(&row, "accordion_open_limit")?.as_str() {
            "one" => OpenGroupLimit::One,
            _ => OpenGroupLimit::Several,
        },
    })
}

async fn write_presentation(
    connection: &turso::Connection,
    presentation: GroupMenuPresentation,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO group_menu_opening_choices
                 (id, layout, cascading_reveal_trigger, accordion_open_limit)
             VALUES (0, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET layout = ?1, cascading_reveal_trigger = ?2,
                 accordion_open_limit = ?3",
            (
                layout_name(presentation.layout),
                open_trigger_name(presentation.open_trigger),
                open_limit_name(presentation.accordion_open_limit),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{read_presentation, write_presentation};
    use crate::group_menus::group_menu_presentation::{
        GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, OpenGroupLimit,
    };

    #[tokio::test]
    async fn an_unwritten_presentation_keeps_its_default() {
        let connection = migrated_in_memory_store().await;
        assert_eq!(
            read_presentation(&connection).await.unwrap(),
            GroupMenuPresentation::default()
        );
    }

    #[tokio::test]
    async fn every_choice_round_trips() {
        let connection = migrated_in_memory_store().await;
        let chosen = GroupMenuPresentation {
            layout: GroupMenuLayout::Accordion,
            open_trigger: GroupMenuOpenTrigger::Press,
            accordion_open_limit: OpenGroupLimit::One,
        };
        write_presentation(&connection, chosen).await.unwrap();
        assert_eq!(read_presentation(&connection).await.unwrap(), chosen);
        write_presentation(&connection, GroupMenuPresentation::default())
            .await
            .unwrap();
        assert_eq!(
            read_presentation(&connection).await.unwrap(),
            GroupMenuPresentation::default()
        );
    }

    #[tokio::test]
    async fn the_table_refuses_an_unknown_layout() {
        let connection = migrated_in_memory_store().await;
        let refused = connection
            .execute(
                "INSERT INTO group_menu_opening_choices (id, layout, cascading_reveal_trigger, \
                 accordion_open_limit) VALUES (0, 'fan', 'hover', 'one')",
                (),
            )
            .await;
        assert!(refused.is_err());
    }
}
