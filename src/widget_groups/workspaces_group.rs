pub mod workspace_tooltip;

use iced::{
    Alignment, Element,
    widget::{Row, button, row},
};

use crate::{
    config::Config,
    data::source::niri,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        activity_line::activity_line, flat_button, focus_indicator::focus_indicator,
        pager_slot::PagerSlot, text::bar_text,
    },
};

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Clicked { id: u64, is_active: bool },
    Tooltip(TooltipMessage),
}

pub fn view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
    position: Position,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let active = workspaces.iter().position(|p| p.is_active);
    let slots = workspaces.iter().map(|p| {
        let label = workspace_label(config, p);
        let focus_opacity = if p.is_urgent { 1.0 } else { 0.0 };
        let focus = activity_line(
            config.palette.urgent,
            focus_opacity,
            config.widgets.focus_underline_thickness_px,
        );
        let ticks = window_ticks(config, niri, p.id);

        let pressable = button(
            PagerSlot::new(label, focus)
                .ticks(ticks)
                .view(config, position),
        )
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
    });
    let row = Row::with_children(slots)
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center);
    let active_is_urgent = active
        .and_then(|i| workspaces.get(i))
        .is_some_and(|p| p.is_urgent);
    let color = if active_is_urgent {
        config.palette.urgent
    } else {
        config.palette.focus
    };
    Some(
        focus_indicator(row, active)
            .color(color)
            .thickness(config.widgets.focus_underline_thickness_px)
            .edge(position)
            .into(),
    )
}

fn window_ticks<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
) -> Element<'a, WorkspacesMessage> {
    let thickness = config.widgets.window_segment_thickness_px;
    let segments = niri.windows_on_workspace(workspace).iter().map(|w| {
        let color = if w.is_urgent {
            config.palette.urgent
        } else if w.is_focused {
            config.palette.focus
        } else {
            config.palette.inactive
        };
        activity_line(color, 1.0, thickness)
    });
    Row::with_children(segments).spacing(1.0).into()
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
