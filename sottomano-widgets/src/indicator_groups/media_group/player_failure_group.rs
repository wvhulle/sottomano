use sottomano_desktop_sources::media_playback_audio::mpris_players::{Media, mpris_player_name};

use super::{PlayerInteraction, PlayerSession, player_interaction};
use crate::{
    group_indicators::snapshot_absence::AbsenceCause,
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
            IndicatorGroupTitle,
        },
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
};

fn unreadable_player_glyph() -> IconGlyph {
    Phosphor::MusicNotes(Weight::Regular).into()
}

pub(super) fn player_failure_group<'a>(
    bus: &'a str,
    read_failure: &'a AbsenceCause<Media>,
    session: &'a PlayerSession,
) -> IndicatorGroupContent<'a> {
    let player_name = mpris_player_name(bus);
    IndicatorGroupContent::expandable_instance(
        IndicatorGroupIdentity {
            family: IndicatorGroupFamily::Media,
            instance: IndicatorGroupInstance::Session {
                durable: player_name.to_owned().into(),
                session: bus.to_owned().into(),
            },
        },
        IndicatorGroupTitle::Named(player_name.to_owned().into()),
        read_failure.indicator(unreadable_player_glyph()),
        &session.expansion,
        move |expansion| player_interaction(bus, PlayerInteraction::Expansion(expansion)),
        [read_failure.indicator(unreadable_player_glyph())],
    )
}
