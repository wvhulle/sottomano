//! The niri workspace pager (odin's `Workspaces` ConcreteWidget/WidgetGroup).
//! Stateless — always renders live niri state, so unlike battery/system it has
//! no owned fields.

use iced::{
    Alignment, Element,
    widget::{Row, mouse_area, row},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::niri,
    layer_surface::Position,
    user_config::Config,
    visual_elements::{bar_text, edge_slot, hairline},
};

/// The workspace pager (odin's `Workspaces`): this output's workspaces in index
/// order, as clickable slots. Focus/urgency show as a hairline along the
/// *inner* edge of each slot (`focus` / `urgent`, hidden otherwise), and the
/// windows on each workspace show as a row of thin tick segments along the
/// *outer* edge — never as label tinting, so labels stay neutral. Clicking a
/// workspace focuses it.
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

            // Inner-edge focus/urgent hairline — always present, opacity conveys state.
            let (focus_color, focus_opacity) = if p.is_urgent {
                (config.palette.urgent(), 1.0)
            } else if p.is_active {
                (config.palette.focus(), 1.0)
            } else {
                (config.palette.focus(), 0.0)
            };
            let focus = hairline(
                focus_color,
                focus_opacity,
                config.layout.focus_underline_thickness,
            );

            // Outer-edge per-window tick segments for this workspace.
            let ticks = window_ticks(config, niri, p.id);

            mouse_area(edge_slot(position, label, focus, Some(ticks)))
                .on_press(Message::FocusWorkspace(p.id))
                .into()
        });
    Row::with_children(slots)
        .spacing(config.layout.horizontal_widget_spacing as f32 * 1.5)
        .align_y(Alignment::Center)
        .into()
}

/// odin's `WorkspaceWindowIndicators`: one thin tick per window on the
/// workspace, equal widths across the slot. Colour: urgent → `urgent`,
/// focused → `focus`, otherwise `inactive`. The active/inactive workspace
/// distinction for non-focused ticks is dropped — the focus hairline on the
/// slot itself carries that state.
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
            hairline(color, 1.0, thickness)
        });
    Row::with_children(segments).spacing(1.0).into()
}

/// A workspace's neutral label (odin's slot content): a named workspace in the
/// keyboard-reachable range (index ≤ 10, the 1–9/0 keys) gets a small raised
/// index prefix before its name; otherwise just the name, or the bare index
/// when unnamed.
fn workspace_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, Message> {
    // Workspace 10 is bound to "0"; above 10 the number is useless as a hint.
    let key = if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => {
            // Smaller, dim, top-aligned index reads as a superscript before the name.
            row![
                bar_text(key).color(config.palette.dim()),
                bar_text(name.clone()),
            ]
            .align_y(Alignment::Start)
            .into()
        }
        Some(name) if named => bar_text(name.clone()).into(),
        _ => bar_text(key).into(),
    }
}
