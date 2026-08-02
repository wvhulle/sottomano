use crate::{
    config::resolved::ResolvedConfig,
    data::{
        notifications::model::NotificationData,
        source::niri::{HEdge, Placement, VEdge},
    },
};

pub struct ToastPlacement {
    pub output: Option<String>,
    pub vertical: VEdge,
    pub horizontal: HEdge,
}

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
        let edges = config.notifications.fallback_position.edges();
        return ToastPlacement {
            output: fallback_output(placement),
            vertical: edges.vertical,
            horizontal: edges.horizontal,
        };
    };

    let edge = placement.window_edge(id);
    let output = edge.output.or_else(|| fallback_output(placement));

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

fn fallback_output(placement: &Placement) -> Option<String> {
    placement
        .focused_window()
        .and_then(|w| placement.workspace_of_window(w))
        .and_then(|ws| placement.output_of_workspace(ws).map(str::to_owned))
}
