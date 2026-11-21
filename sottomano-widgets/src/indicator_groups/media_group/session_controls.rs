use corredo::press_actions::ConfirmStage;
use sottomano_desktop_sources::media_playback_audio::mpris_players::{
    Command, CommandOffer, Media, Status,
};
use sottomano_toml_config::Config;

use super::{PlayerInteraction, PlayerSession, lifted_to_player, player_interaction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    hover_cards::{MouseGesture, PressAction},
    indicator_groups::media_group::confirm_button::configured_confirm_button,
};

fn stop_indicator(media: &Media) -> Option<GroupIndicator<'_>> {
    let offers_stop =
        media.capabilities.stop == CommandOffer::Offered && media.status != Status::Stopped;
    offers_stop.then(|| {
        let state = IndicatorState::Off;
        GroupIndicator::glyph_only("Stop", Phosphor::Stop(state.weight()).into(), state).on_press(
            "stop playback",
            player_interaction(&media.bus, PlayerInteraction::Command(Command::Stop)),
        )
    })
}

fn quit_indicator<'a>(
    config: &'a Config,
    media: &'a Media,
    session: &'a PlayerSession,
) -> Option<GroupIndicator<'a>> {
    (media.capabilities.quit == CommandOffer::Offered).then(|| {
        let bus = media.bus.as_str();
        let quit_does = match session.quit_button.stage {
            ConfirmStage::Armed => "confirm quitting the player",
            ConfirmStage::Idle => "quit the player",
        };
        GroupIndicator::composed(
            "Quit",
            IndicatorIcon::Absent,
            IndicatorValue::drawn(move |reveal| {
                lifted_to_player(
                    bus,
                    configured_confirm_button(
                        &session.quit_button,
                        config,
                        reveal,
                        Phosphor::Power(Weight::Regular),
                        PlayerInteraction::Quit,
                        PlayerInteraction::CancelQuit,
                    ),
                )
            }),
            Box::new(|_| Vec::new()),
        )
        .with_button_actions([PressAction::new(MouseGesture::LeftClick, quit_does)])
    })
}

pub(super) fn session_button_indicators<'a>(
    config: &'a Config,
    media: &'a Media,
    session: &'a PlayerSession,
) -> impl Iterator<Item = GroupIndicator<'a>> {
    stop_indicator(media)
        .into_iter()
        .chain(quit_indicator(config, media, session))
}
