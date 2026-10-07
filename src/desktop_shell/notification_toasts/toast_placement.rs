use iced::{Element, Rectangle};

use super::toast_lifecycle::NotificationMessage;
use crate::{
    config::Config,
    data::{
        notifications::model::NotificationData,
        source::niri::{HEdge, Placement, VEdge, WindowOwner},
    },
    state::OverlaySettings,
};

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
    pub fn is_window_bound(self) -> bool {
        matches!(self, Self::Window { .. })
    }

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
    pub output: Option<String>,
    pub anchor: ToastAnchor,
}

pub struct AnchoredToast<'a> {
    pub id: u32,
    pub anchor: ToastAnchor,
    pub card: Element<'a, NotificationMessage>,
    pub animating: bool,
}

pub fn resolve_window_id(data: &NotificationData, placement: &Placement) -> Option<u64> {
    data.window_id_hint.or_else(|| {
        data.desktop_entry
            .clone()
            .and_then(|entry| placement.window_owned_by(&WindowOwner::App(entry)))
    })
}

pub fn placement_for(
    data: &NotificationData,
    placement: &Placement,
    overlays: OverlaySettings,
    config: &Config,
) -> ToastPlacement {
    let id = overlays
        .toast_spatial_layout
        .then(|| resolve_window_id(data, placement))
        .flatten()
        .filter(|&id| placement.workspace_of_window(id).is_some());

    let Some(id) = id else {
        let fallback_corner = config.toasts.placement.fallback_corner;
        return ToastPlacement {
            output: fallback_output(placement),
            anchor: ToastAnchor::Edge {
                vertical: fallback_corner.vertical_edge(),
                horizontal: fallback_corner.horizontal_edge(),
            },
        };
    };

    let edge = placement.window_edge(id);
    let output = edge.output.or_else(|| fallback_output(placement));
    let anchor = match placement.window_rect(id) {
        Some(bounds) => ToastAnchor::Window { id, bounds },
        None => ToastAnchor::Edge {
            vertical: edge.vertical,
            horizontal: edge.horizontal,
        },
    };
    ToastPlacement { output, anchor }
}

fn fallback_output(placement: &Placement) -> Option<String> {
    placement
        .focused_window()
        .and_then(|w| placement.workspace_of_window(w))
        .and_then(|ws| placement.output_of_workspace(ws))
        .or_else(|| placement.focused_output())
        .map(str::to_owned)
}
