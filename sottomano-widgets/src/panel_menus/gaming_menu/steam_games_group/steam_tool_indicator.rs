use std::cmp::Reverse;

use chrono::Locale;
use sottomano_desktop_sources::steam_client_files::{
    steam_app_manifests::InstalledSteamApp, steam_library_usage::SteamLibraryFolderUsage,
};

use super::{LibraryEntry, SteamGamesInteraction, install_size_indicator::size_text};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::ReadoutLabel,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

fn steam_tool_indicator<'library>(
    installed_app: &'library InstalledSteamApp,
    folder_usage: &'library SteamLibraryFolderUsage,
    locale: Locale,
) -> GroupIndicator<'library> {
    GroupIndicator::glyph_only(
        installed_app.name.clone(),
        Phosphor::Wrench(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(size_text(
        installed_app.size_on_disk_bytes,
        locale,
    )))
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Installed",
            dictionary_table([
                LabeledValue::new(
                    "Folder",
                    installed_app.install_directory.display().to_string(),
                ),
                LabeledValue::new(
                    "Free on its drive",
                    size_text(folder_usage.free_bytes, locale),
                ),
                LabeledValue::new("App id", installed_app.app_id.number.to_string()),
            ]),
        )]
    }))
}

fn tools_summary_indicator<'library>(
    tools_bytes: u64,
    tool_count: usize,
    locale: Locale,
) -> GroupIndicator<'library> {
    GroupIndicator::glyph_only(
        "Steam tools",
        Phosphor::Wrench(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(format!(
        "{}, {tool_count} tools",
        size_text(tools_bytes, locale)
    )))
}

pub(super) fn tools_group<'library>(
    tool_entries: impl Iterator<Item = LibraryEntry<'library>>,
    tools_expansion: &'library IndicatorGroupExpansion,
    locale: Locale,
) -> Option<IndicatorGroupContent<'library>> {
    let mut tools: Vec<LibraryEntry<'library>> = tool_entries.collect();
    if tools.is_empty() {
        return None;
    }
    tools.sort_by_key(|entry| Reverse(entry.installed_app.size_on_disk_bytes));
    let tools_bytes = tools
        .iter()
        .map(|entry| entry.installed_app.size_on_disk_bytes)
        .sum();
    let headline_indicator = tools_summary_indicator(tools_bytes, tools.len(), locale)
        .with_headline_glyph(Phosphor::Wrench(Weight::Regular).into());
    Some(IndicatorGroupContent::expandable(
        IndicatorGroupFamily::SteamTools,
        headline_indicator,
        tools_expansion,
        SteamGamesInteraction::ToolsExpansion,
        IndicatorListing::Listed(
            tools
                .into_iter()
                .map(|entry| steam_tool_indicator(entry.installed_app, entry.folder_usage, locale))
                .collect(),
        ),
    ))
}
