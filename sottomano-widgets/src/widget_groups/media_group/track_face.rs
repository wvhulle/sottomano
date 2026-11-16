use corredo::theming::LINE_HEIGHT;
use iced::widget::{rich_text, span, text::Span};
use sottomano_desktop_sources::{
    media_playback_audio::mpris_players::{Command, Media, Status},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_toml_config::Config;
use uom::si::f64::Ratio;

use super::{PlayerInteraction, RAISE_PLAYER_HINT, player_interaction};
use crate::{
    group_indicators::{
        gauge::Gauge,
        group_indicator::{GroupIndicator, IndicatorValue},
        indicator_drawing::IndicatorIcon,
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        text::font_faces_of,
    },
};

fn playback_state(media: &Media) -> IndicatorState {
    if media.status == Status::Playing {
        IndicatorState::Idle
    } else {
        IndicatorState::Off
    }
}

pub(super) fn track_item<'a>(
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

pub(super) fn player_item<'a>(
    config: &'a Config,
    media: &'a Media,
    gauge_fraction: Option<Ratio>,
) -> GroupIndicator<'a> {
    let state = playback_state(media);
    let glyph = SimpleIcon::titled(&media.identity)
        .map_or_else(|| Phosphor::Radio(Weight::Regular).into(), IconGlyph::from);
    let value = gauge_fraction.map_or(IndicatorValue::Nothing, |fraction| {
        IndicatorValue::drawn(move |reveal| {
            let tint = IndicatorStateTint {
                state,
                opacity: reveal,
            };
            Gauge::new(fraction, tint, config.theme.font.icon_size_px).into()
        })
    });
    GroupIndicator::composed(
        "Player",
        IndicatorIcon::IconGlyph { glyph, state },
        value,
        Box::new(|_| Vec::new()),
    )
}

pub(super) fn label_spans<'a>(config: &Config, media: &Media) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![span(title.clone()).font(font_faces_of(&config.theme.font).regular)]
}
