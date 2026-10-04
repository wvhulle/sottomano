use std::collections::HashMap;

use iced::{Padding, Rectangle, Task, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::{
    actions::ActionCallback,
    reexport::{
        Anchor, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption,
    },
};

use crate::{
    data::source::niri,
    desktop_shell::{DesktopShell, Message},
    message_subscriptions::blocking,
    state::niri_rules::StyledSurface,
    widgets::region_reporter::ReportedRegion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Left,
}

impl Position {
    pub(crate) const BARS: [Position; 2] = [Position::Top, Position::Bottom];
    pub(crate) const ALL: [Position; 3] = [Position::Top, Position::Bottom, Position::Left];

    pub(crate) fn edge(self) -> Anchor {
        match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
            Position::Left => Anchor::Left,
        }
    }

    pub(crate) fn anchor(self) -> Anchor {
        match self {
            Position::Top | Position::Bottom => self.edge() | Anchor::Left | Anchor::Right,
            Position::Left => self.edge() | Anchor::Top | Anchor::Bottom,
        }
    }

    pub(crate) fn strip_size(self, thickness: u32) -> LayerSize {
        match self {
            Position::Top | Position::Bottom => LayerSize::fill_width(thickness),
            Position::Left => LayerSize::fill_height(thickness),
        }
    }

    pub(crate) fn is_vertical(self) -> bool {
        matches!(self, Position::Left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Panel,
    Reservation(Position),
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub role: SurfaceRole,
}

#[derive(Debug, Clone)]
pub struct PanelEdge {
    pub output: String,
    pub position: Position,
}

pub(crate) fn input_region(surface: Id, bounds: Rectangle) -> Message {
    input_regions(surface, vec![bounds])
}

pub(crate) fn input_regions(surface: Id, rects: Vec<Rectangle>) -> Message {
    Message::SetInputRegion {
        id: surface,
        callback: ActionCallback::new(move |region| {
            rects
                .iter()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
                .for_each(|rect| {
                    region.add(
                        rect.x as i32,
                        rect.y as i32,
                        rect.width.ceil() as i32,
                        rect.height.ceil() as i32,
                    );
                });
        }),
    }
}

pub(crate) fn base_settings(output: Option<&str>, namespace: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: output.map_or(OutputOption::Active, |name| {
            OutputOption::OutputName(name.to_string())
        }),
        namespace: Some(namespace.to_string()),
        ..Default::default()
    }
}

fn panel_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Top,
        ..base_settings(Some(output), StyledSurface::Panel.namespace())
    }
}

fn reservation_settings(output: &str, position: Position, zone: i32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: position.strip_size(1),
        exclusive_zone: Some(zone),
        anchor: position.anchor(),
        layer: Layer::Bottom,
        events_transparent: true,
        ..base_settings(Some(output), "vetrata-reservation")
    }
}

fn backdrop_margin(position: Position, height: u32) -> Margin {
    let offscreen = -height.cast_signed();
    match position {
        Position::Top => Margin {
            top: offscreen,
            ..Margin::default()
        },
        Position::Bottom => Margin {
            bottom: offscreen,
            ..Margin::default()
        },
        Position::Left => Margin {
            left: offscreen,
            ..Margin::default()
        },
    }
}

fn backdrop_settings(output: &str, position: Position, height: u32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: position.strip_size(height),
        exclusive_zone: Some(-1),
        anchor: position.anchor(),
        layer: Layer::Background,
        margin: Some(backdrop_margin(position, height)),
        events_transparent: true,
        ..base_settings(Some(output), StyledSurface::Backdrop(position).namespace())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellElement {
    TopBar,
    TopBarStart,
    TopBarCenter,
    TopBarEnd,
    BottomBar,
    BottomBarStart,
    BottomBarMiddle,
    BottomBarEnd,
    WorkspaceSidebar,
    NotificationToasts,
}

#[derive(Default)]
pub struct ReportedElementRegions {
    by_element: HashMap<(Id, ShellElement), ReportedRegion>,
}

impl ReportedElementRegions {
    fn report(
        &mut self,
        panel: Id,
        element: ShellElement,
        region: ReportedRegion,
    ) -> Task<Message> {
        self.by_element.insert((panel, element), region);
        self.input_region_of(panel)
    }

    fn width_of(&self, panel: Id, element: ShellElement) -> f32 {
        self.by_element
            .get(&(panel, element))
            .into_iter()
            .flat_map(|region| &region.rects)
            .map(|rect| rect.width)
            .fold(0.0, f32::max)
    }

    fn forget(&mut self, window: Id) {
        self.by_element.retain(|(panel, _), _| *panel != window);
    }

    fn regions_on(&self, panel: Id) -> impl Iterator<Item = &ReportedRegion> {
        self.by_element
            .iter()
            .filter(move |((owner, _), _)| *owner == panel)
            .map(|(_, region)| region)
    }

    fn input_region_of(&self, panel: Id) -> Task<Message> {
        let rects = self
            .regions_on(panel)
            .filter(|region| region.roles.takes_input)
            .flat_map(|region| region.rects.iter().copied())
            .collect();
        Task::done(input_regions(panel, rects))
    }
}

impl DesktopShell {
    pub(crate) fn apply_output_change(&mut self, change: OutputChange) -> Task<Message> {
        match change {
            OutputChange::Connected(Some(name)) => {
                Task::batch([self.open_monitor_surfaces_once(&name), query_outputs()])
            }
            OutputChange::Disconnected(Some(name)) => {
                Task::batch([self.forget_output(&name), query_outputs()])
            }
            OutputChange::Rearranged => query_outputs(),
            OutputChange::Unavailable(reason) => {
                tracing::error!("wayland: output tracking stopped: {reason}");
                Task::none()
            }
            OutputChange::Connected(None) | OutputChange::Disconnected(None) => Task::none(),
        }
    }

    fn forget_output(&mut self, output: &str) -> Task<Message> {
        let stranded: Vec<Id> = self
            .surfaces
            .iter()
            .filter(|(_, surface)| surface.output == output)
            .map(|(&id, _)| id)
            .collect();
        self.tooltip.forget_output(output);
        Task::batch(stranded.into_iter().flat_map(|id| {
            [
                self.forget_window(id),
                Task::done(Message::RemoveWindow(id)),
            ]
        }))
    }

    pub(crate) fn forget_window(&mut self, window: Id) -> Task<Message> {
        self.surfaces.remove(&window);
        self.backdrops.remove(&window);
        self.tooltip.forget(window);
        self.element_regions.forget(window);
        Task::none()
    }

    pub(crate) fn update_element_regions(
        &mut self,
        window: Id,
        element: ShellElement,
        region: ReportedRegion,
    ) -> Task<Message> {
        let regions = self.element_regions.report(window, element, region);
        if element == ShellElement::WorkspaceSidebar {
            return Task::batch([regions, self.reserve()]);
        }
        regions
    }

    pub(crate) fn panels(&self) -> impl Iterator<Item = (Id, &str)> {
        self.surfaces
            .iter()
            .filter(|(_, surface)| surface.role == SurfaceRole::Panel)
            .map(|(&id, surface)| (id, surface.output.as_str()))
    }

    pub(crate) fn bar_clearance(&self, output: &str) -> Padding {
        Padding::ZERO
            .top(self.reserved_zone(output, Position::Top) as f32)
            .bottom(self.reserved_zone(output, Position::Bottom) as f32)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reserved_zone(&self, output: &str, position: Position) -> i32 {
        let thickness = match position {
            Position::Top | Position::Bottom => self.config.bar.height_px,
            Position::Left => self
                .panels()
                .find(|(_, panel_output)| *panel_output == output)
                .map_or(0.0, |(panel, _)| {
                    self.element_regions
                        .width_of(panel, ShellElement::WorkspaceSidebar)
                }),
        };
        if thickness <= 0.0 {
            return 0;
        }
        (self.shell_settings.layout.float_offset_px + thickness).ceil() as i32
    }

    pub(crate) fn reserve(&self) -> Task<Message> {
        Task::batch(
            self.surfaces
                .iter()
                .filter_map(|(&id, surface)| match surface.role {
                    SurfaceRole::Reservation(position) => {
                        Some(Task::done(Message::ExclusiveZoneChange {
                            id,
                            zone_size: self.reserved_zone(&surface.output, position),
                        }))
                    }
                    SurfaceRole::Panel => None,
                })
                .collect::<Vec<_>>(),
        )
    }

    fn open_panel_and_reservations(&mut self, output: &str) -> Task<Message> {
        let panel = Id::unique();
        self.surfaces.insert(
            panel,
            Surface {
                output: output.to_string(),
                role: SurfaceRole::Panel,
            },
        );
        let mut opens = vec![
            Task::done(Message::NewLayerShell {
                settings: panel_settings(output),
                id: panel,
            }),
            Task::done(input_regions(panel, Vec::new())),
        ];
        for position in Position::ALL {
            let id = Id::unique();
            let zone = self.reserved_zone(output, position);
            self.surfaces.insert(
                id,
                Surface {
                    output: output.to_string(),
                    role: SurfaceRole::Reservation(position),
                },
            );
            opens.push(Task::done(Message::NewLayerShell {
                settings: reservation_settings(output, position, zone),
                id,
            }));
        }
        Task::batch(opens)
    }

    fn open_backdrop(&mut self, output: &str, position: Position) -> Task<Message> {
        let height = self.config.bar.height_px as u32;
        let settings = backdrop_settings(output, position, height);
        let id = Id::unique();
        self.backdrops.insert(id, position);
        Task::done(Message::NewLayerShell { settings, id })
    }

    pub(crate) fn resize_backdrops(&self) -> Task<Message> {
        let height = self.config.bar.height_px as u32;
        Task::batch(self.backdrops.iter().flat_map(move |(&id, &position)| {
            [
                Task::done(Message::LayoutChange {
                    id,
                    anchor: position.anchor(),
                    size: position.strip_size(height),
                }),
                Task::done(Message::MarginChange {
                    id,
                    margin: backdrop_margin(position, height),
                }),
            ]
        }))
    }

    fn open_monitor_surfaces_once(&mut self, output: &str) -> Task<Message> {
        let _span = tracing::debug_span!("open monitor surfaces", output).entered();
        if self
            .surfaces
            .values()
            .any(|surface| surface.output == output)
        {
            return Task::none();
        }
        let mut opens = vec![self.open_panel_and_reservations(output)];
        opens.push(self.tooltip.open_surface(output));
        opens.extend(
            Position::BARS
                .into_iter()
                .map(|position| self.open_backdrop(output, position)),
        );
        Task::batch(opens)
    }
}

#[derive(Debug, Clone)]
pub enum OutputChange {
    Connected(Option<String>),
    Rearranged,
    Disconnected(Option<String>),
    Unavailable(String),
}

pub(crate) fn query_outputs() -> Task<Message> {
    blocking("niri outputs", niri::output_info).map(Message::OutputsResolved)
}
