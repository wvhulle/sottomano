//! Niri workspace pager.

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area, row},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::niri,
    layer_surface::Position,
    user_config::{Config, font},
    widgets::{activity_line, bar_text, pager_slot},
};

pub fn view<'a>(
    niri: &EventStreamState,
    output: &str,
    config: &Config,
    position: Position,
) -> Element<'a, Message> {
    let slots = niri::workspaces_for_output(niri, output)
        .into_iter()
        .map(|p| {
            let label = workspace_label(config, &p);
            let (focus_color, focus_opacity) = if p.is_urgent {
                (config.palette.urgent(), 1.0)
            } else if p.is_active {
                (config.palette.focus(), 1.0)
            } else {
                (config.palette.focus(), 0.0)
            };
            let focus = activity_line(
                focus_color,
                focus_opacity,
                config.layout.focus_underline_thickness,
            );
            let ticks = window_ticks(config, niri, p.id);

            mouse_area(pager_slot(position, label, focus, Some(ticks)))
                .on_press(Message::FocusWorkspace(p.id))
                .into()
        });
    Row::with_children(slots)
        .spacing(config.layout.horizontal_widget_spacing * 1.5)
        .align_y(Alignment::Center)
        .into()
}

fn window_ticks<'a>(
    config: &Config,
    niri: &EventStreamState,
    workspace: u64,
) -> Element<'a, Message> {
    let thickness = config.layout.window_segment_thickness;
    let segments = niri::windows_on_workspace(niri, workspace)
        .into_iter()
        .map(move |w| {
            let color = if w.is_urgent {
                config.palette.urgent()
            } else if w.is_focused {
                config.palette.focus()
            } else {
                config.palette.inactive()
            };
            activity_line(color, 1.0, thickness)
        });
    Row::with_children(segments).spacing(1.0).into()
}

fn workspace_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, Message> {
    let key = if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => row![
            bar_text(key)
                .size(font::size() * 0.7)
                .color(config.palette.dim()),
            bar_text(name.clone()),
        ]
        .align_y(Alignment::Start)
        .into(),
        Some(name) if named => bar_text(name.clone()).into(),
        _ => bar_text(key).into(),
    }
}
