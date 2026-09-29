use core::f32::consts::FRAC_1_SQRT_2;
use std::collections::BTreeMap;

use iced::{
    Element, Length, Rectangle, Task,
    alignment::Vertical,
    widget::{Column, Space, column, container, stack},
    window::Id,
};
use iced_exwlshell::reexport::{
    Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::{Bar, Message},
    config::Config,
    data::source::niri,
    state::ChromeState,
    wayland_layers::{
        blur_caster::{self, Zone},
        surface::{Position, Surface, input_region},
    },
    widget_groups::workspaces_group::{self, WorkspacesMessage},
    widgets::{bounds_probe::bounds_probe, overlay_shadow::overlay_shadow},
};

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

pub fn pointer_region(window: Id, zone: Zone, pills: &[Rectangle]) -> Task<Message> {
    if zone != Zone::Sidebar {
        return Task::none();
    }
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
    let Some(workspaces) = workspaces(&bar.niri_snapshot, output, config) else {
        return blur_caster::fill();
    };
    let pill = pill(
        workspaces.map(workspace_message),
        config,
        &bar.session.chrome,
    );
    let measured = bounds_probe(column![pill], move |rects| Message::PillBounds {
        window,
        zone: Zone::Sidebar,
        rects,
    });
    container(measured)
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .into()
}

fn workspaces<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let spacing = config.spacing.widget_spacing_px;
    let slots = workspaces
        .iter()
        .map(|p| workspaces_group::slot(niri, p, config, Position::Left));
    let full_width_hit_areas = workspaces.iter().map(|p| {
        workspaces_group::hit_area(
            p,
            Space::new()
                .width(Length::Fill)
                .height(config.chrome.outer_height_px)
                .into(),
            config,
        )
    });
    Some(
        stack![
            workspaces_group::indicated(
                Column::with_children(slots).spacing(spacing).into(),
                workspaces,
                config,
                Position::Left,
            ),
            Column::with_children(full_width_hit_areas)
                .spacing(spacing)
                .width(Length::Fill),
        ]
        .into(),
    )
}

fn workspace_message(message: WorkspacesMessage) -> Message {
    match message {
        WorkspacesMessage::Clicked { id, is_active } => Message::WorkspaceClicked { id, is_active },
        WorkspacesMessage::Tooltip(tooltip) => Message::Tooltip(tooltip),
    }
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
