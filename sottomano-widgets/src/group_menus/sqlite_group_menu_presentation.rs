use sottomano_sources::sqlite_state_store::{Columns, StoreError, with_table};

use crate::group_menus::group_menu_presentation::{
    GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, HoverCardPlacement,
    OpenGroupLimit,
};

pub const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS group_menu_presentation (
    id INTEGER PRIMARY KEY CHECK (id = 0),
    layout TEXT NOT NULL CHECK (layout IN ('cascading', 'accordion')),
    cascading_reveal_trigger TEXT NOT NULL CHECK (cascading_reveal_trigger IN ('hover', 'press')),
    accordion_open_limit TEXT NOT NULL CHECK (accordion_open_limit IN ('one', 'several')),
    hover_card_placement TEXT NOT NULL CHECK (hover_card_placement IN ('beside_row', \
                                'inline_with_row'))
)";

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

const fn card_placement_name(placement: HoverCardPlacement) -> &'static str {
    match placement {
        HoverCardPlacement::BesideRow => "beside_row",
        HoverCardPlacement::InlineWithRow => "inline_with_row",
    }
}

pub async fn load() -> GroupMenuPresentation {
    with_table(
        "group menu presentation: load failed",
        &[CREATE_TABLE],
        read_presentation,
    )
    .await
    .unwrap_or_default()
}

pub async fn save(presentation: GroupMenuPresentation) -> bool {
    with_table(
        "group menu presentation: save failed",
        &[CREATE_TABLE],
        async |connection| write_presentation(connection, presentation).await,
    )
    .await
    .is_some()
}

async fn read_presentation(
    connection: &turso::Connection,
) -> Result<GroupMenuPresentation, StoreError> {
    let mut rows = connection
        .query(
            "SELECT layout, cascading_reveal_trigger, accordion_open_limit, hover_card_placement
             FROM group_menu_presentation WHERE id = 0",
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
        hover_card_placement: match columns.text(&row, "hover_card_placement")?.as_str() {
            "inline_with_row" => HoverCardPlacement::InlineWithRow,
            _ => HoverCardPlacement::BesideRow,
        },
    })
}

async fn write_presentation(
    connection: &turso::Connection,
    presentation: GroupMenuPresentation,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO group_menu_presentation
                 (id, layout, cascading_reveal_trigger, accordion_open_limit, hover_card_placement)
             VALUES (0, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET layout = ?1, cascading_reveal_trigger = ?2,
                 accordion_open_limit = ?3, hover_card_placement = ?4",
            (
                layout_name(presentation.layout),
                open_trigger_name(presentation.open_trigger),
                open_limit_name(presentation.accordion_open_limit),
                card_placement_name(presentation.hover_card_placement),
            ),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sottomano_sources::sqlite_state_store::in_memory_connection;

    use super::{CREATE_TABLE, read_presentation, write_presentation};
    use crate::group_menus::group_menu_presentation::{
        GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, HoverCardPlacement,
        OpenGroupLimit,
    };

    #[tokio::test]
    async fn an_unwritten_presentation_keeps_its_default() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        assert_eq!(
            read_presentation(&connection).await.unwrap(),
            GroupMenuPresentation::default()
        );
    }

    #[tokio::test]
    async fn every_choice_round_trips() {
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let chosen = GroupMenuPresentation {
            layout: GroupMenuLayout::Accordion,
            open_trigger: GroupMenuOpenTrigger::Press,
            accordion_open_limit: OpenGroupLimit::One,
            hover_card_placement: HoverCardPlacement::InlineWithRow,
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
        let connection = in_memory_connection(&[CREATE_TABLE]).await;
        let refused = connection
            .execute(
                "INSERT INTO group_menu_presentation (id, layout, cascading_reveal_trigger, \
                 accordion_open_limit, hover_card_placement) VALUES (0, 'fan', 'hover', 'one', \
                 'beside_row')",
                (),
            )
            .await;
        assert!(refused.is_err());
    }
}
