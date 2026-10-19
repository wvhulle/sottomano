use iced::{
    Alignment, Element,
    widget::{Row, rich_text, text::Span},
};
use sottomano_config::Config;
use sottomano_sources::{
    media_playback_audio::mpris_players::{Command, Media},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use uom::si::f64::Ratio;

use super::{MediaMessage, MediaTarget, RAISE_PLAYER_HINT};
use crate::{
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    widgets::{
        bar_widget::Glyph,
        gauge::Gauge,
        group_item::GroupItem,
        icon_handle::IconHandle,
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        source_reading::icon_value_gap,
        text::{FontFaces, LINE_HEIGHT, colored_span},
        widget_group::GroupEvent,
    },
};

pub(super) fn track_label<'a>(config: &'a Config, media: &'a Media) -> GroupItem<'a, MediaMessage> {
    let raisable = media.owner != OwnerProcess::Unidentified || media.capabilities.raise;
    let press_hints = PressHints {
        left: raisable.then_some(RAISE_PLAYER_HINT),
        ..PressHints::default()
    };
    let label = GroupItem::element(move |reveal| {
        hoverable(
            rich_text(label_spans(config, media, reveal))
                .size(config.theme.font.text_size_px)
                .line_height(LINE_HEIGHT)
                .into(),
            HoverTarget::Media(MediaTarget::Track),
            press_hints,
            GroupEvent::Tooltip,
        )
    });
    if raisable {
        label.on_press(RAISE_PLAYER_HINT, MediaMessage::Command(Command::Raise))
    } else {
        label
    }
}

pub(super) fn track_face<'a>(
    config: &Config,
    art: Option<&IconHandle>,
    opacity: f32,
    gauge_fraction: Option<Ratio>,
) -> Element<'a, MediaMessage> {
    let side = config.theme.font.icon_size_px;
    let cover = track_cover(config, art, opacity, side);
    let Some(fraction) = gauge_fraction else {
        return cover;
    };
    let color = PaletteColors::from(&config.theme.palette)
        .bar_foreground
        .scale_alpha(opacity);
    Row::with_children([cover, Gauge::new(fraction, color, side).into()])
        .spacing(icon_value_gap(config))
        .align_y(Alignment::Center)
        .into()
}

pub(super) fn track_cover<'a>(
    config: &Config,
    art: Option<&IconHandle>,
    opacity: f32,
    side: f32,
) -> Element<'a, MediaMessage> {
    if let Some(art) = art {
        return art.view(side, opacity);
    }
    Glyph::from(Phosphor::MusicNote(Weight::Regular)).view(
        side,
        PaletteColors::from(&config.theme.palette)
            .bar_foreground
            .scale_alpha(opacity),
    )
}

pub(super) fn label_spans<'a>(config: &Config, media: &Media, reveal: f32) -> Vec<Span<'a>> {
    let title = media.title.as_ref().unwrap_or(&media.identity);
    vec![colored_span(
        title.clone(),
        FontFaces::from(&config.theme.font).regular,
        PaletteColors::from(&config.theme.palette)
            .bar_foreground
            .scale_alpha(reveal),
    )]
}
