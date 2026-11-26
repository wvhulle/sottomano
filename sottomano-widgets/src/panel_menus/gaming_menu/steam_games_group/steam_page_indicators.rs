use sottomano_desktop_sinks::steam_client_pages::SteamClientPage;
use sottomano_desktop_sources::steam_client_files::steam_log_lines::SteamAppId;

use super::SteamGamesInteraction;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::{LabeledValue, ValueCell},
};

const PCGAMINGWIKI_APPID_REDIRECT: &str = "https://www.pcgamingwiki.com/api/appid.php";

fn steam_page_indicator<'game>(
    name: &'static str,
    glyph: Phosphor,
    page: SteamClientPage,
    press_does: &'static str,
) -> GroupIndicator<'game> {
    GroupIndicator::glyph_only(name, glyph.into(), IndicatorState::Idle)
        .on_press(press_does, SteamGamesInteraction::OpenPage(page))
}

pub(super) fn launch_indicator<'game>(app_id: SteamAppId) -> GroupIndicator<'game> {
    steam_page_indicator(
        "Launch",
        Phosphor::Play(Weight::Regular),
        SteamClientPage::GameLaunch(app_id),
        "launch this game through Steam",
    )
}

pub(super) fn controller_config_indicator<'game>(app_id: SteamAppId) -> GroupIndicator<'game> {
    steam_page_indicator(
        "Controller config",
        Phosphor::GameController(Weight::Regular),
        SteamClientPage::ControllerConfiguration(app_id),
        "open Steam's controller configuration for this game",
    )
}

pub(super) fn store_page_indicator<'game>(app_id: SteamAppId) -> GroupIndicator<'game> {
    let pcgamingwiki_url = format!("{PCGAMINGWIKI_APPID_REDIRECT}?appid={}", app_id.number);
    steam_page_indicator(
        "Steam page",
        Phosphor::ArrowSquareOut(Weight::Regular),
        SteamClientPage::GameDetails(app_id),
        "open this game's page in Steam",
    )
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Guides",
            dictionary_table([LabeledValue::new(
                "Fixes and settings",
                ValueCell::Link {
                    label: "PCGamingWiki".to_owned(),
                    url: pcgamingwiki_url.clone(),
                },
            )]),
        )]
    }))
}
