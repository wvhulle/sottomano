use corredo::theming::LINE_HEIGHT;
use iced::{
    Task,
    widget::{rich_text, span, text::Span},
};
use sottomano_desktop_sources::{
    media_playback_audio::{
        mpris_players,
        mpris_players::{Command, Media, Status},
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_toml_config::Config;
use uom::si::f64::Ratio;

use super::{
    MediaMessage, PlayerInteraction, PlayerSession, RAISE_PLAYER_HINT, player_interaction,
};
use crate::{
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::{IndicatorIcon, ReadoutGauge},
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    icon_catalog::icon_handle::IconHandle,
    widgets::{
        subscribed_snapshot::SubscribedSnapshot, text::font_faces_of, value_cells::LabeledValue,
    },
};

fn playback_state(media: &Media) -> IndicatorState {
    if media.status == Status::Playing {
        IndicatorState::Idle
    } else {
        IndicatorState::Off
    }
}

pub(super) fn track_indicator<'a>(
    config: &'a Config,
    media: &'a Media,
    cover_art: Option<&IconHandle>,
) -> GroupIndicator<'a> {
    let raisable = media.owner != OwnerProcess::Unidentified || media.capabilities.raise;
    let icon = cover_art.map_or_else(
        || IndicatorIcon::IconGlyph {
            glyph: Phosphor::VinylRecord(Weight::Regular).into(),
            state: playback_state(media),
        },
        |art| IndicatorIcon::Image(art.clone()),
    );
    let track = GroupIndicator::composed(
        "Track",
        icon,
        IndicatorValue::drawn(move |reveal| {
            rich_text(label_spans(config, media))
                .style(
                    IndicatorStateTint {
                        state: IndicatorState::Idle,
                        opacity: reveal,
                    }
                    .text_style(),
                )
                .size(config.theme.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .into()
        }),
        Box::new(|_| Vec::new()),
    );
    if raisable {
        track.on_press(
            RAISE_PLAYER_HINT,
            player_interaction(&media.bus, PlayerInteraction::Command(Command::Raise)),
        )
    } else {
        track
    }
}

pub(super) fn player_headline_indicator<'a>(
    media: &Media,
    gauge_fraction: Option<Ratio>,
) -> GroupIndicator<'a> {
    let glyph = match SimpleIcon::titled(&media.identity) {
        Some(brand_icon) => IconGlyph::from(brand_icon),
        None => Phosphor::MusicNotes(Weight::Regular).into(),
    };
    let gauge = match gauge_fraction {
        Some(fraction) => ReadoutGauge::Gauged {
            fill: fraction,
            live: fraction,
        },
        None => ReadoutGauge::Ungauged,
    };
    GroupIndicator::glyph_only("Player", glyph, playback_state(media)).with_gauge(gauge)
}

pub(super) fn label_spans<'a>(config: &Config, media: &Media) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![span(title.clone()).font(font_faces_of(&config.theme.font).regular)]
}

pub(super) fn player_hover_card(
    player_snapshot: &SubscribedSnapshot<Media>,
    media: &Media,
) -> Vec<Section> {
    player_snapshot.hover_card_with(vec![Section::new(
        "Status",
        dictionary_table([
            LabeledValue::new("Player", media.identity.clone()),
            LabeledValue::new("State", media.status.label()),
        ]),
    )])
}

pub(super) fn cover_art_load(media: &Media, session: &PlayerSession) -> Option<Task<MediaMessage>> {
    let url = media
        .art_url
        .clone()
        .filter(|_| session.cover_art.is_none())?;
    let bus = media.bus.clone();
    let loading = {
        let url = url.clone();
        async move { mpris_players::load_art(&url).await }
    };
    Some(Task::perform(loading, move |pixels| {
        MediaMessage::ArtLoaded {
            bus: bus.clone(),
            url: url.clone(),
            handle: pixels.as_ref().map(IconHandle::from),
        }
    }))
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::media_playback_audio::mpris_players::Status;

    use super::*;
    use crate::indicator_groups::media_group::test_players::titled_player;

    #[test]
    fn a_spotify_handle_shows_the_spotify_glyph() {
        let spotify = titled_player("Spotify", Status::Playing);
        assert!(matches!(
            player_headline_indicator(&spotify, None).face.icon,
            IndicatorIcon::IconGlyph { glyph, .. } if glyph == IconGlyph::from(SimpleIcon::Spotify)
        ));
    }

    #[test]
    fn an_unbranded_player_handle_shows_music_notes() {
        let unbranded = titled_player("Unbranded test player", Status::Playing);
        assert!(matches!(
            player_headline_indicator(&unbranded, None).face.icon,
            IndicatorIcon::IconGlyph { glyph, .. }
                if glyph == IconGlyph::from(Phosphor::MusicNotes(Weight::Regular))
        ));
    }
}
