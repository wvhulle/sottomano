use std::cmp::Reverse;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sinks::steam_client_pages::SteamClientPage;
use sottomano_desktop_sources::steam_client_files::{
    steam_app_manifests::InstalledSteamApp,
    steam_library_usage::{SteamLibraryFolderUsage, SteamLibraryUsage},
    steam_log_lines::SteamAppId,
};
use uom::si::{f64::Ratio, ratio::ratio};

use super::SteamClientInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{
            IndicatorIcon, IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel,
        },
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorListing,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn size_text(bytes: u64, locale: Locale) -> String {
    Quantity::size(ByteSize::b(bytes)).localized_text(locale)
}

fn library_installed_bytes(library_usage: &SteamLibraryUsage) -> u64 {
    library_usage
        .folders
        .iter()
        .map(SteamLibraryFolderUsage::installed_bytes)
        .sum()
}

fn installed_game_count(folder_usage: &SteamLibraryFolderUsage) -> usize {
    folder_usage
        .library_folder
        .installed_apps
        .iter()
        .filter(|installed_app| installed_app.size_on_disk_bytes > 0)
        .count()
}

fn library_share(installed_app: &InstalledSteamApp, library_bytes: u64) -> Ratio {
    let game_bytes = installed_app.size_on_disk_bytes as f64;
    let total_bytes = library_bytes.max(1) as f64;
    Ratio::new::<ratio>((game_bytes / total_bytes).clamp(0.0, 1.0))
}

fn installed_game_hover_card(
    installed_app: &InstalledSteamApp,
    folder_usage: &SteamLibraryFolderUsage,
    share: Ratio,
    locale: Locale,
) -> Vec<Section> {
    vec![Section::new(
        "Installed game",
        dictionary_table([
            LabeledValue::new(
                "Size on disk",
                size_text(installed_app.size_on_disk_bytes, locale),
            ),
            LabeledValue::new(
                "Share of library",
                Quantity::share(share).localized_text(locale),
            ),
            LabeledValue::new(
                "Install directory",
                installed_app.install_directory.display().to_string(),
            ),
            LabeledValue::new(
                "Free on its drive",
                size_text(folder_usage.free_bytes, locale),
            ),
            LabeledValue::new("App id", installed_app.app_id.number.to_string()),
        ]),
    )]
}

fn installed_game_indicator<'library>(
    installed_app: &'library InstalledSteamApp,
    folder_usage: &'library SteamLibraryFolderUsage,
    library_bytes: u64,
    locale: Locale,
    game_icon: IndicatorIcon,
) -> GroupIndicator<'library> {
    let share = library_share(installed_app, library_bytes);
    GroupIndicator::icon_only(installed_app.name.clone(), game_icon)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(size_text(installed_app.size_on_disk_bytes, locale)),
            gauge: ReadoutGauge::Gauged {
                fill: share,
                live: share,
            },
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| {
            installed_game_hover_card(installed_app, folder_usage, share, locale)
        }))
        .on_press(
            "launch this game through Steam",
            SteamClientInteraction::OpenPage(SteamClientPage::GameLaunch(installed_app.app_id)),
        )
}

fn library_folders_hover_card(library_usage: &SteamLibraryUsage, locale: Locale) -> Vec<Section> {
    library_usage
        .folders
        .iter()
        .map(|folder_usage| {
            Section::new(
                folder_usage
                    .library_folder
                    .folder_path
                    .display()
                    .to_string(),
                dictionary_table([
                    LabeledValue::new(
                        "Installed",
                        size_text(folder_usage.installed_bytes(), locale),
                    ),
                    LabeledValue::new("Games", installed_game_count(folder_usage).to_string()),
                    LabeledValue::new(
                        "Free on its drive",
                        size_text(folder_usage.free_bytes, locale),
                    ),
                ]),
            )
        })
        .collect()
}

fn summary_indicator(library_usage: &SteamLibraryUsage, locale: Locale) -> GroupIndicator<'_> {
    let game_count: usize = library_usage.folders.iter().map(installed_game_count).sum();
    GroupIndicator::glyph_only(
        "Steam library",
        Phosphor::Books(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(format!(
        "{}, {game_count} games",
        size_text(library_installed_bytes(library_usage), locale)
    )))
    .with_hover_card(Box::new(move |_| {
        library_folders_hover_card(library_usage, locale)
    }))
}

pub(super) fn library_group<'library>(
    library_usage: &'library SteamLibraryUsage,
    expansion: &'library IndicatorGroupExpansion,
    locale: Locale,
    game_icon: impl Fn(SteamAppId) -> IndicatorIcon,
) -> Option<IndicatorGroupContent<'library>> {
    if library_usage.folders.is_empty() {
        return None;
    }
    let library_bytes = library_installed_bytes(library_usage);
    let mut installed_games: Vec<(&SteamLibraryFolderUsage, &InstalledSteamApp)> = library_usage
        .folders
        .iter()
        .flat_map(|folder_usage| {
            folder_usage
                .library_folder
                .installed_apps
                .iter()
                .map(move |installed_app| (folder_usage, installed_app))
        })
        .filter(|(_, installed_app)| installed_app.size_on_disk_bytes > 0)
        .collect();
    installed_games.sort_by_key(|(_, installed_app)| Reverse(installed_app.size_on_disk_bytes));
    let headline_indicator = summary_indicator(library_usage, locale)
        .with_headline_glyph(Phosphor::Books(Weight::Regular).into());
    Some(IndicatorGroupContent::expandable(
        IndicatorGroupFamily::SteamLibrary,
        headline_indicator,
        expansion,
        SteamClientInteraction::LibraryExpansion,
        IndicatorListing::Listed(
            installed_games
                .into_iter()
                .map(|(folder_usage, installed_app)| {
                    installed_game_indicator(
                        installed_app,
                        folder_usage,
                        library_bytes,
                        locale,
                        game_icon(installed_app.app_id),
                    )
                })
                .collect(),
        ),
    ))
}
