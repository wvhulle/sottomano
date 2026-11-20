use std::iter;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::steam_client_files::steam_library_usage::{
    SteamLibraryFolderUsage, SteamLibraryUsage,
};

use super::SteamClientInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

const SHOWN_LARGEST_GAMES: usize = 5;

fn size_text(bytes: u64, locale: Locale) -> String {
    Quantity::size(ByteSize::b(bytes)).localized_text(locale)
}

fn folder_hover_card(folder_usage: &SteamLibraryFolderUsage, locale: Locale) -> Vec<Section> {
    let installed_apps = &folder_usage.library_folder.installed_apps;
    let (shown_games, hidden_games) =
        installed_apps.split_at(installed_apps.len().min(SHOWN_LARGEST_GAMES));
    let largest_games = shown_games
        .iter()
        .map(|app| LabeledValue::new(app.name.clone(), size_text(app.size_on_disk_bytes, locale)));
    let hidden_games = (!hidden_games.is_empty()).then(|| {
        LabeledValue::new(format!("{} more", hidden_games.len()), "smaller games")
    });
    vec![
        Section::new(
            "Library folder",
            dictionary_table([
                LabeledValue::new(
                    "Folder",
                    folder_usage.library_folder.folder_path.display().to_string(),
                ),
                LabeledValue::new("Installed", size_text(folder_usage.installed_bytes(), locale)),
                LabeledValue::new("Free on its drive", size_text(folder_usage.free_bytes, locale)),
            ]),
        ),
        Section::new("Largest games", dictionary_table(largest_games.chain(hidden_games))),
    ]
}

fn folder_indicator(folder_usage: &SteamLibraryFolderUsage, locale: Locale) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        folder_usage.library_folder.folder_path.display().to_string(),
        Phosphor::FolderSimple(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(size_text(
        folder_usage.installed_bytes(),
        locale,
    )))
    .with_hover_card(Box::new(move |_| folder_hover_card(folder_usage, locale)))
}

fn summary_indicator(library_usage: &SteamLibraryUsage, locale: Locale) -> GroupIndicator<'_> {
    let installed_bytes = library_usage
        .folders
        .iter()
        .map(SteamLibraryFolderUsage::installed_bytes)
        .sum();
    let game_count: usize = library_usage
        .folders
        .iter()
        .map(|folder_usage| folder_usage.library_folder.installed_apps.len())
        .sum();
    GroupIndicator::glyph_only(
        "Steam library",
        Phosphor::Books(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(format!(
        "{}, {game_count} games",
        size_text(installed_bytes, locale)
    )))
}

pub(super) fn library_group<'library>(
    library_usage: &'library SteamLibraryUsage,
    expansion: &'library IndicatorGroupExpansion,
    locale: Locale,
) -> Option<IndicatorGroupContent<'library>> {
    if library_usage.folders.is_empty() {
        return None;
    }
    let headline_indicator = summary_indicator(library_usage, locale)
        .with_headline_glyph(Phosphor::Books(Weight::Regular).into());
    Some(IndicatorGroupContent::expandable(
        IndicatorGroupFamily::SteamLibrary,
        headline_indicator,
        expansion,
        SteamClientInteraction::LibraryExpansion,
        iter::once(summary_indicator(library_usage, locale)).chain(
            library_usage
                .folders
                .iter()
                .map(|folder_usage| folder_indicator(folder_usage, locale)),
        ),
    ))
}
