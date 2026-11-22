use sottomano_desktop_sources::sqlite_state_store::migrated_in_memory_store;

use super::{
    GroupCollapsing, PanelGroupFraming, PanelLayout, PanelSettingsGroup, PanelSettingsMessage,
    sqlite_panel_layout::{read_layout, write_layout},
};
use crate::indicator_groups::window_group::WindowRowLayout;

#[tokio::test]
async fn panel_layout_survives_a_round_trip() {
    let connection = migrated_in_memory_store().await;
    let stored = PanelLayout {
        group_framing: PanelGroupFraming::Disconnected,
        group_collapsing: GroupCollapsing::StayExpanded,
        float_offset_px: 3.25,
    };
    write_layout(&connection, stored).await.unwrap();
    assert_eq!(read_layout(&connection).await.unwrap(), stored);
}

#[tokio::test]
async fn an_unwritten_panel_layout_keeps_its_defaults() {
    let connection = migrated_in_memory_store().await;
    assert_eq!(
        read_layout(&connection).await.unwrap(),
        PanelLayout::default()
    );
}

#[tokio::test]
async fn the_panel_layout_table_refuses_a_missing_value() {
    let connection = migrated_in_memory_store().await;
    let refused = connection
        .execute(
            "INSERT INTO panel_layout (id, disconnected_groups, auto_collapse_groups, \
             float_offset_px) VALUES (0, NULL, 1, 0.0)",
            (),
        )
        .await;
    assert!(refused.is_err());
}

#[test]
fn windows_are_grouped_by_location_until_a_stored_layout_says_otherwise() {
    let mut panel_settings_group = PanelSettingsGroup::default();
    assert_eq!(
        panel_settings_group.window_row_layout,
        WindowRowLayout::GroupedByLocation
    );
    let _ = panel_settings_group.update(PanelSettingsMessage::WindowRowLayoutLoaded(
        WindowRowLayout::Flat,
    ));
    assert_eq!(
        panel_settings_group.window_row_layout,
        WindowRowLayout::Flat
    );
}

#[test]
fn a_loaded_panel_layout_reads_back_unchanged() {
    let loaded = PanelLayout {
        group_framing: PanelGroupFraming::Disconnected,
        group_collapsing: GroupCollapsing::StayExpanded,
        float_offset_px: 6.0,
    };
    let mut panel_settings_group = PanelSettingsGroup::default();
    let _ = panel_settings_group.update(PanelSettingsMessage::LayoutLoaded(loaded));
    assert_eq!(
        panel_settings_group.group_framing,
        PanelGroupFraming::Disconnected
    );
    assert_eq!(
        panel_settings_group.group_collapsing,
        GroupCollapsing::StayExpanded
    );
    assert!((panel_settings_group.float_offset_px() - 6.0).abs() < f32::EPSILON);
}
