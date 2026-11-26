use sottomano_desktop_sources::media_playback_audio::mpris_players::{Media, mpris_player_name};

use super::{PlayerInteraction, PlayerSession, player_interaction};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
            IndicatorGroupTitle,
        },
        snapshot_absence::AbsenceCause,
    },
};

fn unreadable_player_glyph() -> IconGlyph {
    Phosphor::MusicNotes(Weight::Regular).into()
}

pub(super) fn player_failure_group<'view>(
    bus: &'view str,
    read_failure: &'view AbsenceCause<Media>,
    session: &'view PlayerSession,
) -> IndicatorGroupContent<'view> {
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
        IndicatorListing::Listed(Vec::from([
            read_failure.indicator(unreadable_player_glyph())
        ])),
    )
}
