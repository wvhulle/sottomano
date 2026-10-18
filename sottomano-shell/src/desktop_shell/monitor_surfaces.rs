use iced::{Padding, Task, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_config::screen_edges::Position;
use sottomano_sources::{background_work::blocking, niri};
use sottomano_state::niri_rules::StyledSurface;
use sottomano_widgets::{
    layer_surfaces::{base_settings, input_regions},
    shell_message::{Message, OutputChange},
};

use crate::desktop_shell::DesktopShell;

trait EdgeStripLayer {
    fn anchor(self) -> Anchor;
    fn strip_size(self, thickness: u32) -> LayerSize;
}

impl EdgeStripLayer for Position {
    fn anchor(self) -> Anchor {
        match self {
            Position::Top => Anchor::Top | Anchor::Left | Anchor::Right,
            Position::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
            Position::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
        }
    }

    fn strip_size(self, thickness: u32) -> LayerSize {
        match self {
            Position::Top | Position::Bottom => LayerSize::fill_width(thickness),
            Position::Left => LayerSize::fill_height(thickness),
        }
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
        ..base_settings(Some(output), "sottomano-reservation")
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

impl DesktopShell {
    pub fn apply_output_change(&mut self, change: OutputChange) -> Task<Message> {
        match change {
            OutputChange::Connected(name) => {
                Task::batch([self.open_monitor_surfaces_once(&name), query_outputs()])
            }
            OutputChange::Disconnected(name) => {
                Task::batch([self.forget_output(&name), query_outputs()])
            }
            OutputChange::Rearranged => query_outputs(),
            OutputChange::Unavailable(reason) => {
                tracing::error!("wayland: output tracking stopped: {reason}");
                Task::none()
            }
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

    pub fn forget_window(&mut self, window: Id) -> Task<Message> {
        self.surfaces.remove(&window);
        self.backdrops.remove(&window);
        self.tooltip.forget(window);
        self.sidebar_widths.remove(&window);
        Task::none()
    }

    pub fn update_sidebar_width(&mut self, window: Id, width: f32) -> Task<Message> {
        self.sidebar_widths.insert(window, width);
        self.reserve()
    }

    pub fn panels(&self) -> impl Iterator<Item = (Id, &str)> {
        self.surfaces
            .iter()
            .filter(|(_, surface)| surface.role == SurfaceRole::Panel)
            .map(|(&id, surface)| (id, surface.output.as_str()))
    }

    #[must_use]
    pub fn bar_clearance(&self, output: &str) -> Padding {
        Padding::ZERO
            .top(self.reserved_zone(output, Position::Top) as f32)
            .bottom(self.reserved_zone(output, Position::Bottom) as f32)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reserved_zone(&self, output: &str, position: Position) -> i32 {
        let thickness = match position {
            Position::Top | Position::Bottom => self.config.layout.panel.height,
            Position::Left => self
                .panels()
                .find(|(_, panel_output)| *panel_output == output)
                .and_then(|(panel, _)| self.sidebar_widths.get(&panel).copied())
                .unwrap_or_default(),
        };
        if thickness <= 0.0 {
            return 0;
        }
        (self.shell_settings.layout.float_offset_px + thickness).ceil() as i32
    }

    pub fn reserve(&self) -> Task<Message> {
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
        let height = self.config.layout.panel.height as u32;
        let settings = backdrop_settings(output, position, height);
        let id = Id::unique();
        self.backdrops.insert(id, position);
        Task::done(Message::NewLayerShell { settings, id })
    }

    pub fn resize_backdrops(&self) -> Task<Message> {
        let height = self.config.layout.panel.height as u32;
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

pub fn query_outputs() -> Task<Message> {
    Task::perform(
        blocking("niri outputs", niri::output_info),
        Message::OutputsResolved,
    )
}
