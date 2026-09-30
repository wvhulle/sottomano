use core::f32::consts::FRAC_1_SQRT_2;
use std::collections::BTreeMap;

use iced::{
    Element, Length, Rectangle, Task, Vector,
    alignment::Vertical,
    widget::{column, container},
    window::Id,
};
use iced_exwlshell::reexport::{
    Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::{Bar, Message},
    config::Config,
    state::ChromeState,
    wayland_layers::{
        blur_caster::{self, Zone},
        surface::{Position, Surface, input_region},
    },
    widget_groups::workspaces_group,
    widgets::{bounds_probe::bounds_probe, overlay_shadow::overlay_shadow},
};

pub fn windows(surfaces: &BTreeMap<Id, Surface>) -> Vec<Id> {
    surfaces
        .iter()
        .filter(|(_, surface)| surface.position.is_vertical())
        .map(|(&id, _)| id)
        .collect()
}

pub fn open(
    surfaces: &mut BTreeMap<Id, Surface>,
    output: &str,
    chrome: &ChromeState,
) -> Task<Message> {
    let id = Id::unique();
    surfaces.insert(
        id,
        Surface {
            output: output.to_string(),
            position: Position::Left,
        },
    );
    Task::batch([
        Task::done(Message::NewLayerShell {
            settings: layer_settings(output, chrome),
            id,
        }),
        Task::done(input_region(id, Rectangle::default())),
    ])
}

pub fn pointer_region(window: Id, pills: &[Rectangle]) -> Task<Message> {
    let covered = pills
        .iter()
        .copied()
        .reduce(|covered, pill| covered.union(&pill))
        .unwrap_or_default();
    Task::done(input_region(window, covered))
}

fn layer_settings(output: &str, chrome: &ChromeState) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Top,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(
            Position::Left
                .float_margin(chrome.margin_offset_px())
                .edges(),
        ),
        ..Default::default()
    }
}

pub fn view<'a>(bar: &'a Bar, window: Id, output: &str) -> Element<'a, Message> {
    let config = &bar.config;
    let Some(workspaces) = workspaces_group::vertical_view(&bar.niri_snapshot, output, config)
    else {
        return blur_caster::fill();
    };
    let pill = pill(workspaces.map(Message::from), config, &bar.session.chrome);
    let measured = bounds_probe(column![pill], move |rects| Message::PillBounds {
        window,
        zone: Zone::Sidebar,
        rects,
    });
    container(measured)
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .padding(bar.session.chrome.float_offset_px)
        .into()
}

pub fn origin_in_tooltip_overlay(chrome: &ChromeState) -> Vector {
    Vector::new(-chrome.float_offset_px, -chrome.float_offset_px)
}

fn pill<'a>(
    content: Element<'a, Message>,
    config: &Config,
    chrome: &ChromeState,
) -> Element<'a, Message> {
    let background = config
        .palette
        .surface
        .scale_alpha(chrome.chrome_tint_factor);
    let foreground = config.palette.bar_foreground;
    let radius = chrome.radius();
    let curve_inset = radius * (1.0 - FRAC_1_SQRT_2);
    container(content)
        .padding([curve_inset + config.spacing.group_padding_px, 0.0])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius: radius.into(),
                ..Default::default()
            },
            text_color: Some(foreground),
            shadow: overlay_shadow(1.0),
            ..Default::default()
        })
        .into()
}
