use std::collections::{BTreeMap, HashMap};

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
    state::{PanelLayout, PanelSettings},
    wayland_layers::{
        bar::corner_inset,
        blur_caster::{self, Zone},
        surface::{self, Position, Surface, input_region},
    },
    widget_groups::workspaces_group,
    widgets::bounds_probe::bounds_probe,
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
    reservations: &mut SidebarReservations,
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
    Task::batch([
        Task::done(Message::NewLayerShell {
            settings: layer_settings(output, layout),
            id,
        }),
        Task::done(input_region(id, Rectangle::default())),
        reservations.open(id, output),
    ])
}

pub fn pointer_region(window: Id, pills: &[Rectangle]) -> Task<Message> {
    Task::done(input_region(window, covered(pills)))
}

fn covered(pills: &[Rectangle]) -> Rectangle {
    pills
        .iter()
        .copied()
        .reduce(|covered, pill| covered.union(&pill))
        .unwrap_or_default()
}

fn layer_settings(output: &str, layout: &PanelLayout) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Top,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        margin: Some(
            Position::Left
                .float_margin(layout.margin_offset_px())
                .edges(),
        ),
        ..Default::default()
    }
}

#[derive(Default)]
pub struct SidebarReservations {
    by_sidebar: HashMap<Id, Id>,
}

impl SidebarReservations {
    pub fn contains(&self, id: Id) -> bool {
        self.by_sidebar
            .values()
            .any(|&reservation| reservation == id)
    }

    fn open(&mut self, sidebar: Id, output: &str) -> Task<Message> {
        let id = Id::unique();
        self.by_sidebar.insert(sidebar, id);
        Task::done(Message::NewLayerShell {
            settings: reservation_settings(output),
            id,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn reserve(&self, sidebar: Id, pills: &[Rectangle]) -> Task<Message> {
        let Some(&id) = self.by_sidebar.get(&sidebar) else {
            return Task::none();
        };
        let pill = covered(pills);
        Task::done(Message::ExclusiveZoneChange {
            id,
            zone_size: (pill.x + pill.width).round() as i32,
        })
    }

    pub fn forget(&mut self, sidebar: Id) -> Task<Message> {
        self.by_sidebar
            .remove(&sidebar)
            .map_or_else(Task::none, |id| Task::done(Message::RemoveWindow(id)))
    }
}

fn reservation_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Position::Left.strip_size(1),
        exclusive_zone: Some(0),
        anchor: Position::Left.anchor(),
        layer: Layer::Bottom,
        events_transparent: true,
        ..surface::base_settings(Some(output), "vetrata-sidebar-reservation")
    }
}

pub fn view<'a>(bar: &'a Bar, window: Id, output: &str) -> Element<'a, Message> {
    let config = &bar.config;
    let Some(workspaces) = workspaces_group::vertical_view(&bar.niri_snapshot, output, config)
    else {
        return blur_caster::fill();
    };
    let pill = pill(workspaces.map(Message::from), config, &bar.panel_settings);
    let measured = bounds_probe(column![pill], move |rects| Message::PillBounds {
        window,
        zone: Zone::Sidebar,
        rects,
    });
    container(measured)
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .padding(bar.panel_settings.layout.float_offset_px)
        .into()
}

pub fn origin_in_tooltip_overlay(layout: &PanelLayout) -> Vector {
    Vector::new(-layout.float_offset_px, -layout.float_offset_px)
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
