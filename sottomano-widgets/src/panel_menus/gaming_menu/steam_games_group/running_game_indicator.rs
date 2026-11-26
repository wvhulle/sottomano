use sottomano_desktop_sources::steam_client_files::running_steam_games::RunningSteamGame;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

pub(super) fn running_game_indicator(running_game: &RunningSteamGame) -> GroupIndicator<'_> {
    let launched_at = running_game.launched_at.format("%H:%M").to_string();
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
                LabeledValue::new("Name", running_game.installed_app.name.clone()),
                LabeledValue::new("App id", running_game.app_id.number.to_string()),
                LabeledValue::new("Launched", launched_at.clone()),
            ]),
        )]
    }))
}
