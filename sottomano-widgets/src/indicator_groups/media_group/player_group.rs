use corredo::transitions::ExpansionState;
use sottomano_desktop_sources::{
    desktop_windows_apps::desktop_entries::DesktopEntries,
    media_playback_audio::{lrclib_synced_lyrics::TrackLyrics, mpris_players::Media},
};
use sottomano_toml_config::Config;

use super::{
    PlayerInteraction, PlayerSession, player_interaction,
    seek_bar::{SeekAvailability, SeekBar, seek_position_indicator},
    session_controls::session_button_indicators,
    synced_lyrics_view::lyrics_row,
    track_face::{player_headline_indicator, player_hover_card, track_indicator},
    track_hover_card::track_hover_card,
    transport_controls::transport_button_indicators,
};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupInstance,
            IndicatorGroupTitle,
        },
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

fn player_group_identity(media: &Media, bus: &str) -> IndicatorGroupIdentity {
    IndicatorGroupIdentity {
        family: IndicatorGroupFamily::Media,
        instance: IndicatorGroupInstance::Session {
            durable: media.identity.clone().into(),
            session: bus.to_owned().into(),
        },
    }
}

pub(super) fn player_group<'view>(
    config: &'view Config,
    clock: Clock,
    group_title: IndicatorGroupTitle,
    player_snapshot: &'view SubscribedSnapshot<Media>,
    session: &'view PlayerSession,
    lyrics_snapshot: Option<&'view SubscribedSnapshot<TrackLyrics>>,
    desktop_entries: &'view DesktopEntries,
) -> Option<IndicatorGroupContent<'view>> {
    let media = player_snapshot.get()?;
    let bus = media.bus.as_str();
    let seek_availability = SeekBar::seek_availability(media, clock.instant);
    let gauge_fraction = media
        .progress
        .fraction(media.status, clock.instant)
        .filter(|_| {
            !(seek_availability == SeekAvailability::Seekable
                && session.expansion.state == ExpansionState::Expanded)
        });
    let headline_indicator = player_snapshot
        .indicator(
            |held_media| player_headline_indicator(held_media, gauge_fraction),
            Phosphor::MusicNotes(Weight::Regular).into(),
        )
        .with_hover_card(Box::new(move |_| player_hover_card(player_snapshot, media)));
    let mut rows = vec![
        track_indicator(config, media, &session.cover_art).with_hover_card(Box::new(
            move |card_clock| track_hover_card(player_snapshot, media, desktop_entries, card_clock),
        )),
    ];
    rows.extend(
        lyrics_snapshot
            .and_then(|lyrics_snapshot| lyrics_row(config, lyrics_snapshot, media, clock.instant)),
    );
    if seek_availability == SeekAvailability::Seekable {
        rows.push(seek_position_indicator(config, clock, media, session));
    }
    rows.extend(transport_button_indicators(media));
    rows.extend(session_button_indicators(config, media, session));
    let player_content = IndicatorGroupContent::expandable_instance(
        player_group_identity(media, bus),
        group_title,
        headline_indicator,
        &session.expansion,
        move |expansion| player_interaction(bus, PlayerInteraction::Expansion(expansion)),
        rows,
    );
    Some(match seek_availability {
        SeekAvailability::Seekable => player_content.forwards_scroll("seek in the track"),
        SeekAvailability::Fixed => player_content,
    })
}
