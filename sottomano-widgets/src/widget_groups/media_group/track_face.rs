use corredo::theming::LINE_HEIGHT;
use iced::widget::{rich_text, span, text::Span};
use sottomano_desktop_sources::{
    media_playback_audio::mpris_players::{Command, Media, Status},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_toml_config::Config;
use uom::si::f64::Ratio;

use super::{
    MediaInteraction, MediaTarget, PlayerInteraction, PlayerTarget, RAISE_PLAYER_HINT,
    player_interaction,
};
use crate::{
    group_items::{
        gauge::Gauge,
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
        },
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
    },
    widgets::{
        icon_glyph::IconGlyph,
        icon_handle::IconHandle,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        text::font_faces_of,
    },
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
) -> GroupItem<'a, MediaInteraction> {
    let raisable = media.owner != OwnerProcess::Unidentified || media.capabilities.raise;
    let icon = cover_art.map_or_else(
        || GroupItemIcon::IconGlyph {
            glyph: Phosphor::VinylRecord(Weight::Regular).into(),
            state: playback_state(media),
        },
        |art| GroupItemIcon::Image(art.clone()),
    );
    let track = GroupItem::composed(
        ComposedGroupItemFace {
            name: "Track".into(),
            icon,
        },
        GroupItemHoverArea::WholeItem(PlayerTarget::of(&media.bus, MediaTarget::Track).into()),
        GroupItemValue::drawn(move |reveal| {
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
) -> GroupItem<'a, MediaInteraction> {
    let state = playback_state(media);
    let glyph = SimpleIcon::titled(&media.identity)
        .map_or_else(|| Phosphor::Radio(Weight::Regular).into(), IconGlyph::from);
    let value = gauge_fraction.map_or(GroupItemValue::Nothing, |fraction| {
        GroupItemValue::drawn(move |reveal| {
            let tint = ReadingStateTint {
                state,
                opacity: reveal,
            };
            Gauge::new(fraction, tint, config.theme.font.icon_size_px).into()
        })
    });
    GroupItem::composed(
        ComposedGroupItemFace {
            name: "Player".into(),
            icon: GroupItemIcon::IconGlyph { glyph, state },
        },
        GroupItemHoverArea::WholeItem(PlayerTarget::of(&media.bus, MediaTarget::Player).into()),
        value,
    )
}

pub(super) fn label_spans<'a>(config: &Config, media: &Media) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![span(title.clone()).font(font_faces_of(&config.theme.font).regular)]
}
