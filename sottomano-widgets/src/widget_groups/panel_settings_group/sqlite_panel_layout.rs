use sottomano_desktop_sources::sqlite_state_store::{
    Columns, StoreError, flag, in_state_store, real, with_state_store,
};

use super::{GroupCollapsing, PanelGroupFraming, PanelLayout};

pub(super) async fn load() -> PanelLayout {
    with_state_store("panel layout: load failed", read_layout)
        .await
        .unwrap_or_default()
}

pub(super) async fn save(panel_layout: PanelLayout) -> Result<(), StoreError> {
    in_state_store(async |connection| write_layout(connection, panel_layout).await).await
}

pub(super) async fn read_layout(connection: &turso::Connection) -> Result<PanelLayout, StoreError> {
    let mut rows = connection
        .query(
            "SELECT disconnected_groups, auto_collapse_groups, float_offset_px
             FROM panel_layout WHERE id = 0",
            (),
        )
        .await?;
    let columns = Columns::of(&rows);
    let Some(row) = rows.next().await? else {
        return Ok(PanelLayout::default());
    };
    Ok(PanelLayout {
        group_framing: match columns.integer(&row, "disconnected_groups")? {
            0 => PanelGroupFraming::Joined,
            _ => PanelGroupFraming::Disconnected,
        },
        group_collapsing: match columns.integer(&row, "auto_collapse_groups")? {
            0 => GroupCollapsing::StayExpanded,
            _ => GroupCollapsing::OnFocusLoss,
        },
        float_offset_px: columns.real(&row, "float_offset_px")? as f32,
    })
}

pub(super) async fn write_layout(
    connection: &turso::Connection,
    panel_layout: PanelLayout,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO panel_layout (id, disconnected_groups, auto_collapse_groups, \
             float_offset_px)
             VALUES (0, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET disconnected_groups = ?1, auto_collapse_groups = ?2, \
             float_offset_px = ?3",
            (
                flag(panel_layout.group_framing == PanelGroupFraming::Disconnected),
                flag(panel_layout.group_collapsing == GroupCollapsing::OnFocusLoss),
                real(panel_layout.float_offset_px),
            ),
        )
        .await?;
    Ok(())
}
