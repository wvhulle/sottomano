use std::collections::{BTreeMap, BTreeSet};

use iced::{Padding, Task, window::Id};
pub use iced_exwlshell::reexport::Margin;
use iced_exwlshell::reexport::{Anchor, Layer, LayerSize, NewLayerShellSettings};
use sottomano_desktop_sources::{
    blocking_background_work::blocking, compositor_window_ipc::compositor_connection,
};
use sottomano_toml_config::screen_edges::Position;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoverCardPresence {
    OnThisOutput,
    Elsewhere,
}

impl HoverCardPresence {
    fn on_output(hover_card_output: Option<&str>, output: &str) -> Self {
        if hover_card_output == Some(output) {
            HoverCardPresence::OnThisOutput
        } else {
            HoverCardPresence::Elsewhere
        }
    }
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
    hover_card_output: Option<&str>,
) -> Vec<Message> {
    surfaces
        .iter_mut()
        .filter_map(|(&id, surface)| {
            let layer = surface.role.layer(HoverCardPresence::on_output(
                hover_card_output,
                surface.output.as_str(),
            ));
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

    pub(super) fn hover_card_output(&self) -> Option<&str> {
        self.hover_card
            .placement()
            .map(|placement| placement.output.as_str())
    }

    fn track_new_surface(&mut self, output: &str, role: SurfaceRole) -> TrackedSurface {
        let id = Id::unique();
        let layer = role.layer(HoverCardPresence::on_output(
            self.hover_card_output(),
            output,
        ));
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
        let hover_card_output = self.hover_card_output().map(str::to_owned);
        let restacked = restack_around_hover_card(&mut self.surfaces, hover_card_output.as_deref());
        Task::batch(
            [hover_card_surfaces, toast_surfaces]
                .into_iter()
                .chain(restacked.into_iter().map(Task::done)),
        )
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
        blocking(
            "compositor outputs",
            compositor_connection::compositor_outputs,
        ),
        Message::OutputsResolved,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iced::window::Id;
    use iced_exwlshell::reexport::Layer;
    use sottomano_toml_config::screen_edges::Position;

    use super::{Surface, SurfaceRole, restack_around_hover_card};
    use crate::desktop_shell::monitor_surfaces::HoverCardPresence;

    fn resting_surface(output: &str, role: SurfaceRole) -> Surface {
        Surface {
            output: output.to_owned(),
            role,
            layer: role.layer(HoverCardPresence::Elsewhere),
        }
    }

    fn surfaces_on_two_outputs() -> BTreeMap<Id, Surface> {
        ["DP-1", "DP-2"]
            .into_iter()
            .flat_map(|output| {
                [
                    SurfaceRole::Panel,
                    SurfaceRole::NotificationToasts,
                    SurfaceRole::Reservation(Position::Top),
                ]
                .map(|role| (Id::unique(), resting_surface(output, role)))
            })
            .collect()
    }

    fn layer_of(surfaces: &BTreeMap<Id, Surface>, output: &str, role: SurfaceRole) -> Layer {
        surfaces
            .values()
            .find(|surface| surface.output == output && surface.role == role)
            .expect("the surface exists")
            .layer
    }

    #[test]
    fn a_hover_card_lifts_its_panel_above_the_toasts_of_its_output_only() {
        let mut surfaces = surfaces_on_two_outputs();
        let changes = restack_around_hover_card(&mut surfaces, Some("DP-1"));
        assert_eq!(changes.len(), 2);
        assert_eq!(
            layer_of(&surfaces, "DP-1", SurfaceRole::Panel),
            Layer::Overlay
        );
        assert_eq!(
            layer_of(&surfaces, "DP-1", SurfaceRole::NotificationToasts),
            Layer::Top
        );
        assert_eq!(layer_of(&surfaces, "DP-2", SurfaceRole::Panel), Layer::Top);
        assert_eq!(
            layer_of(&surfaces, "DP-2", SurfaceRole::NotificationToasts),
            Layer::Overlay
        );
    }

    #[test]
    fn an_unchanged_hover_card_requests_no_further_layer_changes() {
        let mut surfaces = surfaces_on_two_outputs();
        let _lifted = restack_around_hover_card(&mut surfaces, Some("DP-1"));
        assert!(restack_around_hover_card(&mut surfaces, Some("DP-1")).is_empty());
    }

    #[test]
    fn closing_the_hover_card_restores_the_resting_layers() {
        let mut surfaces = surfaces_on_two_outputs();
        let _lifted = restack_around_hover_card(&mut surfaces, Some("DP-1"));
        let restored = restack_around_hover_card(&mut surfaces, None);
        assert_eq!(restored.len(), 2);
        assert_eq!(layer_of(&surfaces, "DP-1", SurfaceRole::Panel), Layer::Top);
        assert_eq!(
            layer_of(&surfaces, "DP-1", SurfaceRole::NotificationToasts),
            Layer::Overlay
        );
    }
}
