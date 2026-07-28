//! Niri workspace pager.

use iced::{
    Alignment, Element, Padding,
    widget::{Row, container, mouse_area, row},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::niri,
    wayland_layers::surface::Position,
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, text::bar_text},
};

#[derive(Debug, Clone, Copy)]
pub enum WorkspacesMessage {
    Clicked(u64),
}

pub fn view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &ResolvedConfig,
    position: Position,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let slots = workspaces.iter().map(|p| {
        let label = workspace_label(config, p);
        let (focus_color, focus_opacity) = if p.is_urgent {
            (config.palette.urgent, 1.0)
        } else if p.is_active {
            (config.palette.focus, 1.0)
        } else {
            (config.palette.focus, 0.0)
        };
        let focus = activity_line(
            focus_color,
            focus_opacity,
            config.layout.focus_underline_thickness_px,
        );
        let ticks = window_ticks(config, niri, p.id);

        mouse_area(PagerSlot::new(label, focus).ticks(ticks).view(position))
            .on_press(WorkspacesMessage::Clicked(p.id))
            .into()
    });
    Some(
        Row::with_children(slots)
            .spacing(config.layout.horizontal_widget_spacing_px * 1.5)
            .align_y(Alignment::Center)
            .into(),
    )
}

fn window_ticks<'a>(
    config: &ResolvedConfig,
    niri: &niri::Snapshot,
    workspace: u64,
) -> Element<'a, WorkspacesMessage> {
    let thickness = config.layout.window_segment_thickness_px;
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

fn workspace_label<'a>(
    config: &ResolvedConfig,
    p: &niri::WsPill,
) -> Element<'a, WorkspacesMessage> {
    let key = if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    };
    let named = p.name.as_deref().is_some_and(|n| !n.is_empty());

    let label: Element<'a, WorkspacesMessage> = match &p.name {
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
    };

    // Drop the label by the font's descent so its glyphs sit centred between the
    // inner hairline and the outer window ticks. iced centres the full line box
    // (ascent + descent + leading), so a label with no descenders sits high by
    // exactly the reserved descent; padding the top by that cancels it.
    container(label)
        .padding(Padding {
            top: config.font.descent_px,
            ..Padding::ZERO
        })
        .into()
}
