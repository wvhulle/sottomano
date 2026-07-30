//! Where a toast renders: which output and corner. Ports odin's
//! `NotificationPlacement` — the toast points back toward its producer window,
//! falling back to the configured corner only when no window resolves.

use crate::{
    config::resolved::ResolvedConfig,
    data::{
        notifications::model::NotificationData,
        source::niri::{HEdge, Placement, VEdge},
    },
};

pub struct ToastPlacement {
    /// The output to bind the surface to, or `None` to let the compositor pick
    /// the active output (no window resolved and no focused output).
    pub output: Option<String>,
    pub vertical: VEdge,
    pub horizontal: HEdge,
}

/// The window a notification is about: the `x-niri-window-id` hint, else a
/// best-effort app-id/title match against live niri windows (via `Placement`,
/// which carries the window identities).
pub fn resolve_window_id(data: &NotificationData, placement: &Placement) -> Option<u64> {
    if let Some(hint) = data.window_id_hint {
        return Some(hint);
    }
    let mut app_ids = Vec::new();
    if let Some(entry) = &data.desktop_entry {
        app_ids.push(entry.clone());
    }
    if !data.app_name.is_empty() {
        app_ids.push(data.app_name.clone());
    }
    let title = (!data.app_name.is_empty()).then_some(data.app_name.as_str());
    placement.window_by_source(&app_ids, title)
}

pub fn placement_for(
    data: &NotificationData,
    placement: &Placement,
    config: &ResolvedConfig,
) -> ToastPlacement {
    let id = config
        .notifications
        .point_at_source
        .then(|| resolve_window_id(data, placement))
        .flatten()
        .filter(|&id| placement.workspace_of_window(id).is_some());

    let Some(id) = id else {
        let (vertical, horizontal) = config.notifications.fallback_position.edges();
        return ToastPlacement {
            output: fallback_output(placement),
            vertical,
            horizontal,
        };
    };

    let edge = placement.window_edge(id);
    let output = edge.output.or_else(|| fallback_output(placement));

    // The sole focus-sensitive placement: the producer is the window you are
    // looking at, so the toast goes dead centre on its screen.
    if placement.focused_window() == Some(id) {
        ToastPlacement {
            output,
            vertical: VEdge::Center,
            horizontal: HEdge::Center,
        }
    } else {
        ToastPlacement {
            output,
            vertical: edge.vertical,
            horizontal: edge.horizontal,
        }
    }
}

/// The output to fall back to when a notification names no window: the one the
/// focused window is on (show it where the user is looking), else `None` (the
/// compositor's active output).
fn fallback_output(placement: &Placement) -> Option<String> {
    placement
        .focused_window()
        .and_then(|w| placement.workspace_of_window(w))
        .and_then(|ws| placement.output_of_workspace(ws).map(str::to_owned))
}
