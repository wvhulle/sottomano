use std::iter;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sinks::steam_client_pages::SteamClientPage;
use sottomano_desktop_sources::steam_client_files::running_steam_games::RunningSteamGame;

use super::SteamClientInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorIcon, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::{
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn launch_indicator(game: &RunningSteamGame) -> GroupIndicator<'_> {
    let launched_at = game.launched_at.format("%H:%M").to_string();
    GroupIndicator::glyph_only(
        "Running game",
        Phosphor::Play(Weight::Regular).into(),
        IndicatorState::On,
    )
    .with_label(ReadoutLabel::Labeled(format!("since {launched_at}")))
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Game",
            dictionary_table([
                LabeledValue::new("Name", game.installed_app.name.clone()),
                LabeledValue::new("App id", game.app_id.number.to_string()),
                LabeledValue::new("Launched", launched_at.clone()),
            ]),
        )]
    }))
}

fn processes_indicator(game: &RunningSteamGame) -> GroupIndicator<'_> {
    let process_count = game.tracked_processes.len();
    GroupIndicator::glyph_only(
        "Processes",
        Phosphor::TreeStructure(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(format!("{process_count} processes")))
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Tracked by Steam",
            dictionary_table(game.tracked_processes.iter().map(|process| {
                LabeledValue::new(process.process_id.to_string(), process.command_name.clone())
            })),
        )]
    }))
}

fn install_indicator(game: &RunningSteamGame, locale: Locale) -> GroupIndicator<'_> {
    let size_on_disk = Quantity::size(ByteSize::b(game.installed_app.size_on_disk_bytes));
    GroupIndicator::glyph_only(
        "Install size",
        Phosphor::HardDrives(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(size_on_disk.localized_text(locale)))
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Installed",
            dictionary_table([
                LabeledValue::new(
                    "Folder",
                    game.installed_app.install_directory.display().to_string(),
                ),
                LabeledValue::new("Size on disk", size_on_disk),
            ]),
        )]
    }))
}

fn steam_page_indicator<'game>(
    name: &'static str,
    glyph: Phosphor,
    page: SteamClientPage,
    press_does: &'static str,
) -> GroupIndicator<'game> {
    GroupIndicator::glyph_only(name, glyph.into(), IndicatorState::Idle)
        .on_press(press_does, SteamClientInteraction::OpenPage(page))
}

pub(super) fn running_game_group<'game>(
    game: &'game RunningSteamGame,
    expansion: &'game IndicatorGroupExpansion,
    locale: Locale,
    game_icon: IndicatorIcon,
) -> IndicatorGroupContent<'game> {
    let app_id = game.app_id;
    let headline_indicator = launch_indicator(game).with_icon(game_icon);
    IndicatorGroupContent::expandable_instance(
        IndicatorGroupIdentity::durable(
            IndicatorGroupFamily::SteamRunningGame,
            app_id.number.to_string(),
        ),
        IndicatorGroupTitle::Named(game.installed_app.name.clone().into()),
        headline_indicator,
        expansion,
        move |expansion| SteamClientInteraction::RunningGameExpansion {
            app_number: app_id.number,
            expansion,
        },
        iter::once(launch_indicator(game)).chain([
            processes_indicator(game),
            install_indicator(game, locale),
            steam_page_indicator(
                "Controller config",
                Phosphor::GameController(Weight::Regular),
                SteamClientPage::ControllerConfiguration(app_id),
                "open Steam's controller configuration for this game",
            ),
            steam_page_indicator(
                "Steam page",
                Phosphor::ArrowSquareOut(Weight::Regular),
                SteamClientPage::GameDetails(app_id),
                "open this game's page in Steam",
            ),
        ]),
    )
}
