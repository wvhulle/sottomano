use corredo::popovers::{
    NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
    SubmenuConnector,
};
use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, in_state_store, with_state_store,
};

const fn submenu_connector_name(submenu_connector: SubmenuConnector) -> &'static str {
    match submenu_connector {
        SubmenuConnector::Hidden => "hidden",
        SubmenuConnector::Shown => "shown",
    }
}

const fn layout_name(layout: NestedMenuLayout) -> &'static str {
    match layout {
        NestedMenuLayout::Cascading => "cascading",
        NestedMenuLayout::Accordion => "accordion",
    }
}

const fn open_trigger_name(trigger: NestedMenuOpenTrigger) -> &'static str {
    match trigger {
        NestedMenuOpenTrigger::Hover => "hover",
        NestedMenuOpenTrigger::Press => "press",
    }
}

const fn open_limit_name(limit: OpenGroupLimit) -> &'static str {
    match limit {
        OpenGroupLimit::One => "one",
        OpenGroupLimit::Several => "several",
    }
}

pub async fn load() -> NestedMenuPresentation {
    with_state_store("group menu presentation: load failed", read_presentation)
        .await
        .unwrap_or_default()
}

pub async fn save(presentation: NestedMenuPresentation) -> Result<(), StoreError> {
    in_state_store(async |connection| write_presentation(connection, presentation).await).await
}

async fn read_presentation(
    connection: &turso::Connection,
) -> Result<NestedMenuPresentation, StoreError> {
    let mut rows = connection
        .query(
            "SELECT layout, cascading_reveal_trigger, accordion_open_limit, submenu_connector
             FROM group_menu_opening_choices WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(NestedMenuPresentation::default());
    };
    Ok(NestedMenuPresentation {
        layout: match columns.text(&row, "layout")?.as_str() {
            "accordion" => NestedMenuLayout::Accordion,
            _ => NestedMenuLayout::Cascading,
        },
        open_trigger: match columns.text(&row, "cascading_reveal_trigger")?.as_str() {
            "press" => NestedMenuOpenTrigger::Press,
            _ => NestedMenuOpenTrigger::Hover,
        },
        accordion_open_limit: match columns.text(&row, "accordion_open_limit")?.as_str() {
            "one" => OpenGroupLimit::One,
            _ => OpenGroupLimit::Several,
        },
        submenu_connector: match columns.text(&row, "submenu_connector")?.as_str() {
            "shown" => SubmenuConnector::Shown,
            _ => SubmenuConnector::Hidden,
        },
    })
}

async fn write_presentation(
    connection: &turso::Connection,
    presentation: NestedMenuPresentation,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO group_menu_opening_choices
                 (id, layout, cascading_reveal_trigger, accordion_open_limit, submenu_connector)
             VALUES (0, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET layout = ?1, cascading_reveal_trigger = ?2,
                 accordion_open_limit = ?3, submenu_connector = ?4",
            (
                layout_name(presentation.layout),
                open_trigger_name(presentation.open_trigger),
                open_limit_name(presentation.accordion_open_limit),
                submenu_connector_name(presentation.submenu_connector),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use corredo::popovers::{
        NestedMenuLayout, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
        SubmenuConnector,
    };
    use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

    use super::{read_presentation, write_presentation};

    #[tokio::test]
    async fn an_unwritten_presentation_keeps_its_default() {
        let connection = migrated_in_memory_store().await;
        assert_eq!(
            read_presentation(&connection).await.unwrap(),
            NestedMenuPresentation::default()
        );
    }

    #[tokio::test]
    async fn every_choice_round_trips() {
        let connection = migrated_in_memory_store().await;
        let chosen = NestedMenuPresentation {
            layout: NestedMenuLayout::Accordion,
            open_trigger: NestedMenuOpenTrigger::Press,
            accordion_open_limit: OpenGroupLimit::One,
            submenu_connector: SubmenuConnector::Shown,
        };
        write_presentation(&connection, chosen).await.unwrap();
        assert_eq!(read_presentation(&connection).await.unwrap(), chosen);
        write_presentation(&connection, NestedMenuPresentation::default())
            .await
            .unwrap();
        assert_eq!(
            read_presentation(&connection).await.unwrap(),
            NestedMenuPresentation::default()
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
