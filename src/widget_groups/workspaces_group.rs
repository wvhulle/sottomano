pub mod workspace_preview;
pub mod workspace_tooltip;

use iced::{
    Alignment, Color, Element, Length, padding,
    widget::{Column, Row, Space, Text, button, column, container, row, stack},
};

use crate::{
    config::Config,
    data::source::niri,
    desktop_shell::{
        Message,
        hover_tooltips::{HoverTarget, TooltipMessage, hoverable},
        monitor_surfaces::Position,
    },
    widgets::{
        activity_line::edge_activity_line,
        flat_button,
        focus_indicator::focus_indicator,
        marker_track::MarkerTrack,
        pager_slot::PagerSlot,
        text::bar_text,
        window_width_strip::{WidthSegment, WindowWidthStrip},
    },
};

const SUPERSCRIPT_SCALE: f32 = 0.7;

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Clicked { id: u64, is_active: bool },
    Tooltip(TooltipMessage),
}

impl From<WorkspacesMessage> for Message {
    fn from(message: WorkspacesMessage) -> Self {
        match message {
            WorkspacesMessage::Clicked { id, is_active } => {
                Message::WorkspaceClicked { id, is_active }
            }
            WorkspacesMessage::Tooltip(tooltip) => Message::Tooltip(tooltip),
        }
    }
}

pub fn vertical_view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let rows = workspaces.iter().map(|p| sidebar_row(config, p));
    let labels = indicated(
        Column::with_children(rows)
            .spacing(config.spacing.widget_spacing_px)
            .align_x(Alignment::End)
            .into(),
        workspaces,
        config,
        Position::Left,
    );
    let full_width_windows = Column::with_children(
        workspaces
            .iter()
            .map(|p| hit_area(p, sidebar_windows(config, niri, p.id), config)),
    )
    .spacing(config.spacing.widget_spacing_px);
    Some(stack![labels, full_width_windows].into())
}

fn sidebar_windows<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
) -> Element<'a, WorkspacesMessage> {
    let urgency_line_clearance =
        config.spacing.widget_spacing_px + config.widget_sizes.indicator_thickness_px;
    container(window_widths(config, niri, workspace))
        .padding(padding::right(urgency_line_clearance))
        .align_right(Length::Fill)
        .align_bottom(config.bar.height_px)
        .clip(true)
        .into()
}

fn sidebar_row<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
) -> Element<'a, WorkspacesMessage> {
    let window_strip_slot = Space::new().height(config.widget_sizes.indicator_thickness_px);
    let label_over_windows = column![inline_label(config, p), window_strip_slot];
    row![label_over_windows, urgency_line(config, p, Position::Left)]
        .spacing(config.spacing.widget_spacing_px)
        .height(config.bar.height_px)
        .align_y(Alignment::End)
        .into()
}

pub fn inline_view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
    position: Position,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let slots = workspaces.iter().map(|p| {
        let slot = PagerSlot::new(inline_label(config, p), urgency_line(config, p, position))
            .ticks(window_markers(config, niri, p.id, position).view())
            .view(config, position);
        hit_area(p, slot, config)
    });
    Some(indicated(
        Row::with_children(slots)
            .spacing(config.spacing.widget_spacing_px)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into(),
        workspaces,
        config,
        position,
    ))
}

fn indicated<'a>(
    slots: Element<'a, WorkspacesMessage>,
    workspaces: &[niri::WorkspaceIndicator],
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
        .thickness(config.widget_sizes.indicator_thickness_px)
        .edge(position)
        .into()
}

fn hit_area<'a>(
    p: &niri::WorkspaceIndicator,
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

fn window_color(config: &Config, w: &niri::WinEntry, unfocused: Color) -> Color {
    if w.is_urgent {
        config.palette.urgent
    } else if w.is_focused {
        config.palette.focus
    } else {
        unfocused
    }
}

fn window_markers(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
    position: Position,
) -> MarkerTrack {
    let colors = niri
        .windows_on_workspace(workspace)
        .iter()
        .map(|w| window_color(config, w, config.palette.inactive))
        .collect();
    MarkerTrack::new(colors, config.widget_sizes.indicator_thickness_px, position)
}

fn window_widths(config: &Config, niri: &niri::Snapshot, workspace: u64) -> WindowWidthStrip {
    let segments = niri
        .windows_on_workspace(workspace)
        .iter()
        .map(|w| WidthSegment {
            width_px: w.layout.tile_size.width * config.widget_sizes.window_width_strip_scale,
            color: window_color(config, w, config.palette.dim),
        })
        .collect();
    WindowWidthStrip::new(segments, config.widget_sizes.indicator_thickness_px)
}

fn urgency_line<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let opacity = if p.is_urgent { 1.0 } else { 0.0 };
    edge_activity_line(
        position,
        config.palette.urgent,
        opacity,
        config.widget_sizes.indicator_thickness_px,
    )
}

fn given_name(p: &niri::WorkspaceIndicator) -> Option<&str> {
    p.name.as_deref().filter(|name| !name.is_empty())
}

fn display_name(p: &niri::WorkspaceIndicator) -> String {
    given_name(p).map_or_else(|| p.idx.to_string(), str::to_string)
}

fn has_superscript(p: &niri::WorkspaceIndicator) -> bool {
    given_name(p).is_some() && p.idx <= 9
}

fn superscript<'a>(config: &Config, p: &niri::WorkspaceIndicator) -> Text<'a> {
    bar_text(&config.font, p.idx.to_string())
        .size(config.font.text_size_px * SUPERSCRIPT_SCALE)
        .color(config.palette.inactive)
}

fn inline_label<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
) -> Element<'a, WorkspacesMessage> {
    let name = bar_text(&config.font, display_name(p));
    if !has_superscript(p) {
        return name.into();
    }
    row![superscript(config, p), name]
        .align_y(Alignment::Start)
        .into()
}
