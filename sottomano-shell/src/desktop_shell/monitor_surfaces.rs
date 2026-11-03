use std::collections::BTreeSet;

use iced::{Padding, Task, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_config::screen_edges::Position;
use sottomano_sources::{
    blocking_background_work::blocking, desktop_windows_apps::niri_output_facts,
};
use sottomano_widgets::{
    layer_surfaces::{base_settings, input_regions},
    widget_groups::niri_window_group::niri_kdl_rules::StyledSurface,
};

use crate::{
    desktop_shell::DesktopShell,
    shell_message::{Message, OutputChange},
};

fn panel_edge_anchor(position: Position) -> Anchor {
    match position {
        Position::Top => Anchor::Top | Anchor::Left | Anchor::Right,
        Position::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        Position::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
    }
}

fn panel_edge_strip_size(position: Position, thickness: u32) -> LayerSize {
    match position {
        Position::Top | Position::Bottom => LayerSize::fill_width(thickness),
        Position::Left => LayerSize::fill_height(thickness),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Panel,
    NotificationToasts,
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

fn notification_toasts_settings(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Overlay,
        ..base_settings(Some(output), StyledSurface::NotificationToasts.namespace())
    }
}

fn reservation_settings(output: &str, position: Position, zone: i32) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: panel_edge_strip_size(position, 1),
        exclusive_zone: Some(zone),
        anchor: panel_edge_anchor(position),
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
        size: panel_edge_strip_size(position, height),
        exclusive_zone: Some(-1),
        anchor: panel_edge_anchor(position),
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
        self.hover_card.forget_output(output);
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
        self.hover_card.forget(window);
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

    pub fn notification_toast_surfaces(&self) -> impl Iterator<Item = (Id, &str)> {
        self.surfaces
            .iter()
            .filter(|(_, surface)| surface.role == SurfaceRole::NotificationToasts)
            .map(|(&id, surface)| (id, surface.output.as_str()))
    }

    #[must_use]
    pub fn panel_clearance(&self, output: &str) -> Padding {
        Padding::ZERO
            .top(self.reserved_zone(output, Position::Top) as f32)
            .bottom(self.reserved_zone(output, Position::Bottom) as f32)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reserved_zone(&self, output: &str, position: Position) -> i32 {
        let thickness = match position {
            Position::Top | Position::Bottom => self.config.desktop_panels.height,
            Position::Left => self
                .panels()
                .find(|(_, panel_output)| *panel_output == output)
                .and_then(|(panel, _)| self.sidebar_widths.get(&panel).copied())
                .unwrap_or_default(),
        };
        if thickness <= 0.0 {
            return 0;
        }
        (self.widget_groups.panel_settings_group.float_offset_px() + thickness).ceil() as i32
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
                    SurfaceRole::Panel | SurfaceRole::NotificationToasts => None,
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
            Task::done(input_regions(panel, Vec::new()).into()),
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

    fn open_notification_toasts_surface(&mut self, output: &str) -> Task<Message> {
        let id = Id::unique();
        self.surfaces.insert(
            id,
            Surface {
                output: output.to_string(),
                role: SurfaceRole::NotificationToasts,
            },
        );
        Task::batch([
            Task::done(Message::NewLayerShell {
                settings: notification_toasts_settings(output),
                id,
            }),
            Task::done(input_regions(id, Vec::new()).into()),
        ])
    }

    pub fn reconcile_overlay_surfaces(&mut self) -> Task<Message> {
        let panel_outputs: BTreeSet<String> =
            self.panels().map(|(_, output)| output.to_owned()).collect();
        let hover_card_surfaces = self
            .hover_card
            .reconcile_surfaces(
                &panel_outputs,
                self.clock.instant,
                self.config.overlay_surfaces.surface_morph_duration,
            )
            .map(Message::from);
        Task::batch([
            hover_card_surfaces,
            self.reconcile_notification_toasts_surfaces(),
        ])
    }

    fn reconcile_notification_toasts_surfaces(&mut self) -> Task<Message> {
        let occupied_outputs: BTreeSet<String> = self
            .notifications
            .occupied_outputs()
            .filter(|output| {
                self.panels()
                    .any(|(_, panel_output)| panel_output == *output)
            })
            .map(str::to_owned)
            .collect();
        let emptied_surfaces: Vec<Id> = self
            .notification_toast_surfaces()
            .filter(|(_, output)| !occupied_outputs.contains(*output))
            .map(|(id, _)| id)
            .collect();
        let closes = emptied_surfaces.into_iter().map(|id| {
            self.surfaces.remove(&id);
            Task::done(Message::RemoveWindow(id))
        });
        let closes: Vec<Task<Message>> = closes.collect();
        let missing_outputs: Vec<String> = occupied_outputs
            .into_iter()
            .filter(|output| {
                !self
                    .notification_toast_surfaces()
                    .any(|(_, surface_output)| surface_output == output)
            })
            .collect();
        let opens: Vec<Task<Message>> = missing_outputs
            .iter()
            .map(|output| self.open_notification_toasts_surface(output))
            .collect();
        Task::batch(closes.into_iter().chain(opens))
    }

    fn open_backdrop(&mut self, output: &str, position: Position) -> Task<Message> {
        let height = self.config.desktop_panels.height as u32;
        let settings = backdrop_settings(output, position, height);
        let id = Id::unique();
        self.backdrops.insert(id, position);
        Task::done(Message::NewLayerShell { settings, id })
    }

    pub fn resize_backdrops(&self) -> Task<Message> {
        let height = self.config.desktop_panels.height as u32;
        Task::batch(self.backdrops.iter().flat_map(move |(&id, &position)| {
            [
                Task::done(Message::LayoutChange {
                    id,
                    anchor: panel_edge_anchor(position),
                    size: panel_edge_strip_size(position, height),
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
        blocking("niri outputs", niri_output_facts::output_info),
        Message::OutputsResolved,
    )
}
