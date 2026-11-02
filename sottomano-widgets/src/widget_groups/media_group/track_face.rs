use iced::widget::{rich_text, span, text::Span};
use sottomano_config::Config;
use sottomano_sources::{
    media_playback_audio::mpris_players::{Command, Media, Status},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use uom::si::f64::Ratio;

use super::{
    MediaMessage, MediaTarget, PlayerMessage, PlayerTarget, RAISE_PLAYER_HINT, player_message,
};
use crate::widgets::{
    gauge::Gauge,
    group_item::{GroupItem, ItemHover, ItemIcon, ItemIdentity, ItemValue},
    icon_glyph::IconGlyph,
    icon_handle::IconHandle,
    phosphor::{Phosphor, Weight},
    reading_display_state::{ReadingDisplayState, ReadingStateTint},
    simple_icons::SimpleIcon,
    text::{FontFaces, LINE_HEIGHT},
};

fn playback_state(media: &Media) -> ReadingDisplayState {
    if media.status == Status::Playing {
        ReadingDisplayState::Idle
    } else {
        ReadingDisplayState::Off
    }
}

pub(super) fn track_item<'a>(
    config: &'a Config,
    media: &'a Media,
    cover_art: Option<&IconHandle>,
) -> GroupItem<'a, MediaMessage> {
    let raisable = media.owner != OwnerProcess::Unidentified || media.capabilities.raise;
    let icon = cover_art.map_or_else(
        || ItemIcon::IconGlyph {
            glyph: Phosphor::VinylRecord(Weight::Regular).into(),
            state: playback_state(media),
        },
        |art| ItemIcon::Image(art.clone()),
    );
    let track = GroupItem::composed(
        ItemIdentity {
            name: "Track".into(),
            icon,
        },
        ItemHover::WholeItem(PlayerTarget::of(&media.bus, MediaTarget::Track).into()),
        ItemValue::drawn(move |reveal| {
            rich_text(label_spans(config, media))
                .style(
                    ReadingStateTint {
                        state: ReadingDisplayState::Idle,
                        opacity: reveal,
                    }
                    .text_style(),
                )
                .size(config.theme.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .into()
        }),
    );
    if raisable {
        track.on_press(
            RAISE_PLAYER_HINT,
            player_message(&media.bus, PlayerMessage::Command(Command::Raise)),
        )
    } else {
        track
    }
}

pub(super) fn player_item<'a>(
    config: &'a Config,
    media: &'a Media,
    gauge_fraction: Option<Ratio>,
) -> GroupItem<'a, MediaMessage> {
    let state = playback_state(media);
    let glyph = SimpleIcon::titled(&media.identity)
        .map_or_else(|| Phosphor::Radio(Weight::Regular).into(), IconGlyph::from);
    let value = gauge_fraction.map_or(ItemValue::Nothing, |fraction| {
        ItemValue::drawn(move |reveal| {
            let tint = ReadingStateTint {
                state,
                opacity: reveal,
            };
            Gauge::new(fraction, tint, config.theme.font.icon_size_px).into()
        })
    });
    GroupItem::composed(
        ItemIdentity {
            name: "Player".into(),
            icon: ItemIcon::IconGlyph { glyph, state },
        },
        ItemHover::WholeItem(PlayerTarget::of(&media.bus, MediaTarget::Player).into()),
        value,
    )
}

pub(super) fn label_spans<'a>(config: &Config, media: &Media) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![span(title.clone()).font(FontFaces::from(&config.theme.font).regular)]
}
