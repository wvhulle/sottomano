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

pub(super) fn game_processes_indicator(running_game: &RunningSteamGame) -> GroupIndicator<'_> {
    let process_count = running_game.tracked_processes.len();
    GroupIndicator::glyph_only(
        "Processes",
        Phosphor::TreeStructure(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_label(ReadoutLabel::Labeled(format!("{process_count} processes")))
    .with_hover_card(Box::new(move |_| {
        vec![Section::new(
            "Tracked by Steam",
            dictionary_table(running_game.tracked_processes.iter().map(|process| {
                LabeledValue::new(process.process_id.to_string(), process.command_name.clone())
            })),
        )]
    }))
}
