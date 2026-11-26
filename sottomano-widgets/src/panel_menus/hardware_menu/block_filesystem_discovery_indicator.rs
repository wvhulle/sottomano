use sottomano_desktop_sources::{
    SnapshotSource, disk_storage::block_filesystem_mounts::BlockFilesystemDiscovery,
};

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub(super) fn block_filesystem_discovery_indicator(
    discovery: &BlockFilesystemDiscovery,
) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        BlockFilesystemDiscovery::NAME,
        Phosphor::Folders(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_panel_visibility(PanelVisibility::Hidden)
    .with_hover_card(Box::new(move |_| {
        block_filesystem_discovery_hover_card(discovery)
    }))
}

pub(super) fn block_filesystem_discovery_hover_card(
    discovery: &BlockFilesystemDiscovery,
) -> Vec<Section> {
    vec![Section::new(
        "Mount points",
        dictionary_table(discovery.filesystems.iter().map(|filesystem| {
            LabeledValue::new(
                filesystem.primary_mount_point().display().to_string(),
                filesystem.fs_type.clone(),
            )
        })),
    )]
}
