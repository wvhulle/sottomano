use super::toast_anchor::{ToastAnchor, ToastPlacement};
use crate::{
    config::Config,
    data::{notifications::model::NotificationData, source::niri::Placement},
    state::NotificationState,
};

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
    notif_state: &NotificationState,
    config: &Config,
) -> ToastPlacement {
    let id = notif_state
        .spatial_layout
        .then(|| resolve_window_id(data, placement))
        .flatten()
        .filter(|&id| placement.workspace_of_window(id).is_some());

    let Some(id) = id else {
        let edges = config.notifications.fallback_position.edges();
        return ToastPlacement {
            output: fallback_output(placement),
            anchor: ToastAnchor::Edge {
                vertical: edges.vertical,
                horizontal: edges.horizontal,
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
        .and_then(|ws| placement.output_of_workspace(ws).map(str::to_owned))
}
