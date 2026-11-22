use std::collections::{BTreeMap, BTreeSet};

use iced::{Padding, Task, window::Id};
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_compositor_model::compositor_styling_model::StyledSurface;
use sottomano_compositors::compositor_connection;
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    hover_cards::hover_card_surfaces::HoverCardPresence,
    layer_surfaces::{base_settings, input_regions},
};

use crate::{
    desktop_shell::DesktopShell,
    monitors::edge_backdrops::{panel_edge_anchor, panel_edge_strip_size},
    shell_message::{Message, OutputChange},
};

#[cfg(test)]
mod monitor_surfaces_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Panel,
    NotificationToasts,
    Reservation(Position),
}

impl SurfaceRole {
    fn layer(self, hover_card_presence: HoverCardPresence) -> Layer {
        match (self, hover_card_presence) {
            (Self::Panel, HoverCardPresence::Elsewhere)
            | (Self::NotificationToasts, HoverCardPresence::OnThisOutput) => Layer::Top,
            (Self::Panel, HoverCardPresence::OnThisOutput)
            | (Self::NotificationToasts, HoverCardPresence::Elsewhere) => Layer::Overlay,
            (Self::Reservation(_), _) => Layer::Bottom,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub role: SurfaceRole,
    pub layer: Layer,
}

struct TrackedSurface {
    id: Id,
    layer: Layer,
}

fn restack_around_hover_card(
    surfaces: &mut BTreeMap<Id, Surface>,
    presence_on: impl Fn(&str) -> HoverCardPresence,
) -> Vec<Message> {
    surfaces
        .iter_mut()
        .filter_map(|(&id, surface)| {
            let layer = surface.role.layer(presence_on(surface.output.as_str()));
            (surface.layer != layer).then(|| {
                surface.layer = layer;
                Message::LayerChange { id, layer }
            })
        })
        .collect()
}

fn panel_settings(output: &str, layer: Layer) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer,
        ..base_settings(Some(output), StyledSurface::Panel.namespace())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ToastSurfaceChanges {
    orphaned_surfaces: Vec<Id>,
    missing_outputs: Vec<String>,
}

fn output_surfaces<'outputs>(
    surfaces: impl Iterator<Item = (Id, &'outputs str)>,
) -> Vec<OutputSurface<'outputs>> {
    surfaces
        .map(|(surface, output)| OutputSurface { surface, output })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputSurface<'outputs> {
    surface: Id,
    output: &'outputs str,
}

fn toast_surface_changes(
    panels: &[OutputSurface<'_>],
    toast_surfaces: &[OutputSurface<'_>],
) -> ToastSurfaceChanges {
    let panel_outputs: BTreeSet<&str> = panels.iter().map(|panel| panel.output).collect();
    ToastSurfaceChanges {
        orphaned_surfaces: toast_surfaces
            .iter()
            .filter(|toast_surface| !panel_outputs.contains(toast_surface.output))
            .map(|toast_surface| toast_surface.surface)
            .collect(),
        missing_outputs: panel_outputs
            .into_iter()
            .filter(|panel_output| {
                !toast_surfaces
                    .iter()
                    .any(|toast_surface| toast_surface.output == *panel_output)
            })
            .map(str::to_owned)
            .collect(),
    }
}

fn notification_toasts_settings(output: &str, layer: Layer) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: LayerSize::FILL,
        exclusive_zone: Some(-1),
        anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer,
        ..base_settings(Some(output), StyledSurface::NotificationToasts.namespace())
    }
}

fn reservation_settings(
    output: &str,
    position: Position,
    zone: i32,
    layer: Layer,
) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: panel_edge_strip_size(position, 1),
        exclusive_zone: Some(zone),
        anchor: panel_edge_anchor(position),
        layer,
        events_transparent: true,
        ..base_settings(Some(output), "sottomano-reservation")
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
        (self.indicator_groups.panel_settings_group.float_offset_px() + thickness).ceil() as i32
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

    fn track_new_surface(&mut self, output: &str, role: SurfaceRole) -> TrackedSurface {
        let id = Id::unique();
        let layer = role.layer(self.hover_card.presence_on(output));
        self.surfaces.insert(
            id,
            Surface {
                output: output.to_string(),
                role,
                layer,
            },
        );
        TrackedSurface { id, layer }
    }

    fn open_panel_and_reservations(&mut self, output: &str) -> Task<Message> {
        let panel = self.track_new_surface(output, SurfaceRole::Panel);
        let mut opens = vec![
            Task::done(Message::NewLayerShell {
                settings: panel_settings(output, panel.layer),
                id: panel.id,
            }),
            Task::done(input_regions(panel.id, Vec::new()).into()),
        ];
        for position in Position::ALL {
            let zone = self.reserved_zone(output, position);
            let reservation = self.track_new_surface(output, SurfaceRole::Reservation(position));
            opens.push(Task::done(Message::NewLayerShell {
                settings: reservation_settings(output, position, zone, reservation.layer),
                id: reservation.id,
            }));
        }
        Task::batch(opens)
    }

    fn open_notification_toasts_surface(&mut self, output: &str) -> Task<Message> {
        let TrackedSurface { id, layer } =
            self.track_new_surface(output, SurfaceRole::NotificationToasts);
        Task::batch([
            Task::done(Message::NewLayerShell {
                settings: notification_toasts_settings(output, layer),
                id,
            }),
            Task::done(input_regions(id, Vec::new()).into()),
        ])
    }

    pub fn reconcile_overlay_surfaces(&mut self) -> Task<Message> {
        let panel_surfaces: BTreeMap<String, Id> = self
            .panels()
            .map(|(surface, output)| (output.to_owned(), surface))
            .collect();
        let hover_card_surfaces = self
            .hover_card
            .reconcile_with_panels(
                &panel_surfaces,
                self.clock.instant,
                self.config.overlay_surfaces.surface_morph_duration,
            )
            .map(Message::from);
        let toast_surfaces = self.reconcile_notification_toasts_surfaces();
        let hover_card = &self.hover_card;
        let restacked =
            restack_around_hover_card(&mut self.surfaces, |output| hover_card.presence_on(output));
        Task::batch(
            [hover_card_surfaces, toast_surfaces]
                .into_iter()
                .chain(restacked.into_iter().map(Task::done)),
        )
    }

    fn reconcile_notification_toasts_surfaces(&mut self) -> Task<Message> {
        let ToastSurfaceChanges {
            orphaned_surfaces,
            missing_outputs,
        } = toast_surface_changes(
            &output_surfaces(self.panels()),
            &output_surfaces(self.notification_toast_surfaces()),
        );
        let closes: Vec<Task<Message>> = orphaned_surfaces
            .into_iter()
            .map(|id| {
                if let Some(orphaned) = self.surfaces.remove(&id) {
                    self.hover_card.forget_toast_cards(&orphaned.output);
                }
                Task::done(Message::RemoveWindow(id))
            })
            .collect();
        let opens: Vec<Task<Message>> = missing_outputs
            .iter()
            .map(|output| self.open_notification_toasts_surface(output))
            .collect();
        Task::batch(closes.into_iter().chain(opens))
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
        compositor_connection::compositor_outputs(),
        Message::OutputsResolved,
    )
}
