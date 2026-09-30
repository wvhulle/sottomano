use std::collections::BTreeMap;

use iced::{
    Element, Rectangle, Size, Task, Vector,
    widget::{column, container},
    window::Id,
};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};

use crate::{
    app::{Bar, Message},
    config::Config,
    data::sink::niri_rules::StyledSurface,
    state::{PanelLayout, PanelSettings},
    wayland_layers::{
        bar::corner_inset,
        blur_caster::{self, Zone},
        surface::{self, Position, Surface},
    },
    widget_groups::workspaces_group,
    widgets::bounds_probe::natural_bounds_probe,
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
    layout: &PanelLayout,
) -> Task<Message> {
    let id = Id::unique();
    surfaces.insert(
        id,
        Surface {
            output: output.to_string(),
            position: Position::Left,
        },
    );
    Task::done(Message::NewLayerShell {
        settings: layer_settings(output, layout),
        id,
    })
}

fn layer_settings(output: &str, layout: &PanelLayout) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::px(1, 1),
        exclusive_zone: Some(0),
        anchor: Anchor::Left,
        layer: Layer::Top,
        margin: Some(
            Position::Left
                .float_margin(layout.margin_offset_px())
                .edges(),
        ),
        ..surface::base_settings(Some(output), StyledSurface::Sidebar.namespace())
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn fit(sidebar: Id, pills: &[Rectangle]) -> Task<Message> {
    let Some(pill) = pills.first() else {
        return Task::none();
    };
    let width = pill.width.ceil() as u32;
    let height = pill.height.ceil() as u32;
    Task::batch([
        Task::done(Message::LayoutChange {
            id: sidebar,
            anchor: Anchor::Left,
            size: LayerSize::px(width.max(1), height.max(1)),
        }),
        Task::done(Message::ExclusiveZoneChange {
            id: sidebar,
            zone_size: width.cast_signed(),
        }),
    ])
}

pub fn view<'a>(bar: &'a Bar, window: Id, output: &str) -> Element<'a, Message> {
    let config = &bar.config;
    let Some(workspaces) = workspaces_group::vertical_view(&bar.niri_snapshot, output, config)
    else {
        return blur_caster::fill();
    };
    let pill = pill(workspaces.map(Message::from), config, &bar.panel_settings);
    natural_bounds_probe(column![pill], move |rects| Message::PillBounds {
        window,
        zone: Zone::Sidebar,
        rects,
    })
}

#[allow(clippy::cast_precision_loss)]
pub fn origin_in_tooltip_overlay(bar: &Bar, output: &str) -> impl Fn(Size) -> Vector + use<> {
    let output_height = bar
        .outputs
        .get(output)
        .map_or(0.0, |info| info.logical_height as f32);
    let float_offset = bar.panel_settings.layout.float_offset_px;
    move |sidebar: Size| Vector::new(0.0, (output_height - sidebar.height) / 2.0 - float_offset)
}

fn pill<'a>(
    content: Element<'a, Message>,
    config: &Config,
    panel_settings: &PanelSettings,
) -> Element<'a, Message> {
    let background = config
        .palette
        .surface
        .scale_alpha(panel_settings.layout.tint_factor);
    let foreground = config.palette.bar_foreground;
    let radius = panel_settings.layout.corner_radius;
    container(content)
        .padding([corner_inset(radius) + config.spacing.group_padding_px, 0.0])
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius,
                ..Default::default()
            },
            text_color: Some(foreground),
            ..Default::default()
        })
        .into()
}
