//! The niri workspace pager (odin's `Workspaces` ConcreteWidget/WidgetGroup).
//! Stateless — always renders live niri state, so unlike battery/system it has
//! no owned fields.

use iced::{
    Alignment, Color, Element,
    widget::{Row, mouse_area, row, text, text::Shaping},
};
use niri_ipc::state::EventStreamState;

use crate::{
    bar_program::Message,
    data_sources::niri,
    layer_surface::Position,
    user_config::Config,
    visual_elements::{edge_slot, font_family, hairline},
};

/// The workspace pager (odin's `Workspaces`): this output's workspaces in index
/// order, as clickable slots. Focus/urgency show as a hairline along the
/// *inner* edge of each slot (powerTint / criticalTint, hidden otherwise), and
/// the windows on each workspace show as a row of thin tick segments along the
/// *outer* edge — never as label tinting, so labels stay neutral, matching
/// odin. Clicking a workspace focuses it.
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
                (config.palette.critical_tint.0, 1.0)
            } else if p.is_active {
                (config.palette.power_tint.0, 1.0)
            } else {
                (config.palette.power_tint.0, 0.0)
            };
            let focus = hairline(
                focus_color,
                focus_opacity,
                config.layout.focus_underline_thickness,
            );

            // Outer-edge per-window tick segments for this workspace.
            let ticks = window_ticks(config, niri, p.id, p.is_active);

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
/// workspace (capped so a busy workspace can't outgrow the label), equal widths
/// across the slot. Colour per odin's `WindowSegment`: urgent → criticalTint,
/// focused → powerTint, otherwise white if this is the active workspace, else
/// dimmed white.
fn window_ticks<'a>(
    config: &Config,
    niri: &EventStreamState,
    workspace: u64,
    is_active_workspace: bool,
) -> Element<'a, Message> {
    let thickness = config.layout.window_segment_thickness;
    let segments = niri::windows_on_workspace(niri, workspace)
        .into_iter()
        .take(8)
        .map(move |w| {
            let color = if w.is_urgent {
                config.palette.critical_tint.0
            } else if w.is_focused {
                config.palette.power_tint.0
            } else if is_active_workspace {
                Color::WHITE
            } else {
                Color {
                    a: 0.45,
                    ..Color::WHITE
                }
            };
            // Each tick fills its equal share of the segment row (Fill width).
            hairline(color, 1.0, thickness)
        });
    Row::with_children(segments).spacing(1.0).into()
}

/// A workspace's neutral label (odin's slot content): a named workspace in the
/// keyboard-reachable range (index ≤ 10, the 1–9/0 keys) gets a small raised
/// index prefix before its name; otherwise just the name, or the bare index
/// when unnamed.
fn workspace_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, Message> {
    let size = config.font.size as f32;
    let sans = font_family(&config.font.family);
    // Workspace 10 is bound to "0"; above 10 the number is useless as a hint.
    let key = if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    match &p.name {
        Some(name) if named && p.idx <= 10 => {
            // Smaller, dimmed, top-aligned index reads as a superscript before the name.
            let dim = Color {
                a: 0.5,
                ..Color::WHITE
            };
            row![
                text(key)
                    .size(size * 0.7)
                    .font(sans)
                    .color(dim)
                    .shaping(Shaping::Advanced),
                text(name.clone())
                    .size(size)
                    .font(sans)
                    .shaping(Shaping::Advanced),
            ]
            .align_y(Alignment::Start)
            .into()
        }
        Some(name) if named => text(name.clone())
            .size(size)
            .font(sans)
            .shaping(Shaping::Advanced)
            .into(),
        _ => text(key)
            .size(size)
            .font(sans)
            .shaping(Shaping::Advanced)
            .into(),
    }
}
