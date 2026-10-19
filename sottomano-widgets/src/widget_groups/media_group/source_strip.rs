use iced::{
    Alignment, Element,
    widget::{Row, button, image, svg},
};
use sottomano_config::Config;
use sottomano_sources::media_playback_audio::mpris_players::Media;

use super::{MediaMessage, MediaTarget};
use crate::{
    desktop_index::DesktopIndex,
    hover_tooltips::{PressHints, hoverable},
    widgets::{
        bar_widget::Glyph,
        flat_button,
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        widget_group::GroupEvent,
    },
};

pub(super) fn sources_strip<'a>(
    config: &Config,
    index: &DesktopIndex,
    players: &[&Media],
    current: &Media,
    reveal: f32,
) -> Element<'a, GroupEvent<MediaMessage>> {
    let side = config.theme.font.icon_size_px;
    let icons: Vec<Element<'a, GroupEvent<MediaMessage>>> = players
        .iter()
        .map(|m| {
            let selected = m.bus == current.bus;
            let opacity = reveal * if selected { 1.0 } else { 0.4 };
            let icon = source_icon(config, index, m, side, opacity).map(GroupEvent::Group);
            hoverable(
                button(icon)
                    .padding(0)
                    .style(flat_button(
                        PaletteColors::from(&config.theme.palette).bar_foreground,
                    ))
                    .on_press(GroupEvent::Group(MediaMessage::Source(m.bus.clone())))
                    .into(),
                MediaTarget::Source(m.bus.clone()).into(),
                PressHints {
                    left: (!selected).then_some("control this player"),
                    ..PressHints::default()
                },
                GroupEvent::Tooltip,
            )
        })
        .collect();
    Row::with_children(icons)
        .spacing(config.layout.panel.widget_spacing)
        .align_y(Alignment::Center)
        .into()
}

pub(super) fn source_icon<'a>(
    config: &Config,
    index: &DesktopIndex,
    media: &Media,
    side: f32,
    opacity: f32,
) -> Element<'a, MediaMessage> {
    let icon_path = media
        .desktop_entry
        .as_deref()
        .and_then(|entry| index.app_icon(entry));
    if let Some(path) = icon_path {
        let is_svg = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        return if is_svg {
            svg(svg::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(opacity)
                .into()
        } else {
            image(image::Handle::from_path(path))
                .width(side)
                .height(side)
                .opacity(opacity)
                .into()
        };
    }
    Glyph::from(Phosphor::MusicNote(Weight::Regular)).view(
        side,
        PaletteColors::from(&config.theme.palette)
            .bar_foreground
            .scale_alpha(opacity),
    )
}
