use iced::{Element, Rectangle};
use sottomano_config::{
    Config,
    screen_edges::{HEdge, VEdge},
};
use sottomano_sources::{
    desktop_notifications::notification_data::NotificationData,
    desktop_windows_apps::niri_window_placement::{Placement, WindowOwner},
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};
use sottomano_state::NotificationSettings;

use crate::toast_messages::NotificationMessage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastAnchor {
    Edge { vertical: VEdge, horizontal: HEdge },
    Window { id: u64, bounds: Rectangle },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorGroup {
    Edge { vertical: VEdge, horizontal: HEdge },
    Window(u64),
}

impl ToastAnchor {
    #[must_use]
    pub fn is_window_bound(self) -> bool {
        matches!(self, Self::Window { .. })
    }

    #[must_use]
    pub fn group(self) -> AnchorGroup {
        match self {
            Self::Edge {
                vertical,
                horizontal,
            } => AnchorGroup::Edge {
                vertical,
                horizontal,
            },
            Self::Window { id, .. } => AnchorGroup::Window(id),
        }
    }
}

pub struct ToastPlacement {
    pub window: ToastWindow,
    pub output: Option<String>,
    pub anchor: ToastAnchor,
}

pub struct AnchoredToast<'a> {
    pub id: u32,
    pub anchor: ToastAnchor,
    pub card: Element<'a, NotificationMessage>,
    pub animating: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastWindow {
    Anchored(u64),
    Unanchored,
}

impl ToastWindow {
    pub fn resolve(data: &NotificationData, placement: &Placement) -> Self {
        let resolved = data
            .window_id_hint
            .or_else(|| sender_window(data.sender, placement))
            .or_else(|| {
                data.desktop_entry
                    .clone()
                    .and_then(|entry| placement.window_owned_by(&WindowOwner::App(entry)))
            });
        tracing::debug!("toast {}: window {resolved:?}", data.id);
        resolved.map_or(Self::Unanchored, Self::Anchored)
    }

    #[must_use]
    pub fn open_in(self, placement: &Placement) -> Option<u64> {
        match self {
            Self::Anchored(id) => placement.workspace_of_window(id).map(|_| id),
            Self::Unanchored => None,
        }
    }
}

fn sender_window(sender: OwnerProcess, placement: &Placement) -> Option<u64> {
    match sender {
        OwnerProcess::Identified(pid) => placement.window_owned_by(&WindowOwner::Process(pid)),
        OwnerProcess::Unidentified => None,
    }
}

#[must_use]
pub fn placement_for(
    window: ToastWindow,
    placement: &Placement,
    notifications: NotificationSettings,
    config: &Config,
) -> ToastPlacement {
    let id = notifications
        .toast_spatial_layout
        .then(|| window.open_in(placement))
        .flatten();

    let Some(id) = id else {
        let fallback_corner = config.notification_toasts.placement.fallback_corner;
        return ToastPlacement {
            window,
            output: fallback_output(placement),
            anchor: ToastAnchor::Edge {
                vertical: fallback_corner.vertical_edge(),
                horizontal: fallback_corner.horizontal_edge(),
            },
        };
    };

    let edge = placement.window_edge(id);
    let output = edge.output.or_else(|| fallback_output(placement));
    let viewed_bounds = placement.viewed_window_layout(id).and_then(|layout| {
        let (tile_x, tile_y) = layout.tile_pos_in_workspace_view?;
        let (offset_x, offset_y) = layout.window_offset_in_tile;
        let (width, height) = layout.window_size;
        Some(Rectangle {
            x: (tile_x + offset_x) as f32,
            y: (tile_y + offset_y) as f32,
            width: width as f32,
            height: height as f32,
        })
    });
    let anchor = match viewed_bounds {
        Some(bounds) => ToastAnchor::Window { id, bounds },
        None => ToastAnchor::Edge {
            vertical: edge.vertical,
            horizontal: edge.horizontal,
        },
    };
    ToastPlacement {
        window,
        output,
        anchor,
    }
}

fn fallback_output(placement: &Placement) -> Option<String> {
    placement
        .focused_window()
        .and_then(|w| placement.workspace_of_window(w))
        .and_then(|ws| placement.output_of_workspace(ws))
        .or_else(|| placement.focused_output())
        .map(str::to_owned)
}
