pub mod workspace_preview;
pub mod workspace_tooltip;

use iced::{
    Alignment, Element,
    widget::{Column, Row, button, row},
};

use crate::{
    config::Config,
    data::source::niri,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        activity_line::edge_activity_line, flat_button, focus_indicator::focus_indicator,
        pager_slot::PagerSlot, text::bar_text,
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Clicked { id: u64, is_active: bool },
    Tooltip(TooltipMessage),
}

pub fn slot<'a>(
    niri: &niri::Snapshot,
    p: &niri::WsPill,
    config: &Config,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let focus_opacity = if p.is_urgent { 1.0 } else { 0.0 };
    let focus = edge_activity_line(
        position,
        config.palette.urgent,
        focus_opacity,
        config.widgets.indicator_thickness_px,
    );
    PagerSlot::new(workspace_label(config, p), focus)
        .ticks(window_ticks(config, niri, p.id, position))
        .view(config, position)
}

pub fn indicated<'a>(
    slots: Element<'a, WorkspacesMessage>,
    workspaces: &[niri::WsPill],
    config: &Config,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let active = workspaces.iter().position(|p| p.is_active);
    let active_is_urgent = active
        .and_then(|i| workspaces.get(i))
        .is_some_and(|p| p.is_urgent);
    let color = if active_is_urgent {
        config.palette.urgent
    } else {
        config.palette.focus
    };
    focus_indicator(slots, active, color)
        .thickness(config.widgets.indicator_thickness_px)
        .edge(position)
        .into()
}

pub fn hit_area<'a>(
    p: &niri::WsPill,
    content: Element<'a, WorkspacesMessage>,
    config: &Config,
) -> Element<'a, WorkspacesMessage> {
    let pressable = button(content)
        .padding(0)
        .style(flat_button(config.palette.bar_foreground))
        .on_press(WorkspacesMessage::Clicked {
            id: p.id,
            is_active: p.is_active,
        });
    hoverable(
        pressable.into(),
        HoverTarget::Workspace(p.id),
        WorkspacesMessage::Tooltip,
    )
}

fn window_ticks<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let thickness = config.widgets.indicator_thickness_px;
    let segments = niri.windows_on_workspace(workspace).iter().map(|w| {
        let color = if w.is_urgent {
            config.palette.urgent
        } else if w.is_focused {
            config.palette.focus
        } else {
            config.palette.inactive
        };
        edge_activity_line(position, color, 1.0, thickness)
    });
    if position.is_vertical() {
        Column::with_children(segments).spacing(1.0).into()
    } else {
        Row::with_children(segments).spacing(1.0).into()
    }
}

fn workspace_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    let key = if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => row![
            bar_text(&config.font, key)
                .size(config.font.text_size_px * 0.7)
                .color(config.palette.inactive),
            bar_text(&config.font, name.clone()),
        ]
        .align_y(Alignment::Start)
        .into(),
        Some(name) if named => bar_text(&config.font, name.clone()).into(),
        _ => bar_text(&config.font, key).into(),
    }
}
