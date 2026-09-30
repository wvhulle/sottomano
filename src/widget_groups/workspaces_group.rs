pub mod workspace_preview;
pub mod workspace_tooltip;

use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Column, Space, button, container, row, stack},
};

use crate::{
    config::Config,
    data::source::niri,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage, hoverable},
    },
    widgets::{
        activity_line::edge_activity_line,
        flat_button,
        focus_indicator::focus_indicator,
        text::{LINE_HEIGHT, bar_text},
    },
};

const SUPERSCRIPT_SCALE: f32 = 0.7;

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Clicked { id: u64, is_active: bool },
    Tooltip(TooltipMessage),
}

pub fn view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let list = |cells: Vec<Element<'a, WorkspacesMessage>>| {
        Column::with_children(cells).spacing(config.spacing.widget_spacing_px)
    };
    let ticks = list(
        workspaces
            .iter()
            .map(|p| window_ticks(config, niri, p.id))
            .collect(),
    );
    let keys =
        list(workspaces.iter().map(|p| superscript(config, p)).collect()).align_x(Alignment::End);
    let names = list(workspaces.iter().map(|p| name(config, p)).collect()).align_x(Alignment::End);
    let urgency = list(workspaces.iter().map(|p| urgency_line(config, p)).collect());
    let hit_areas =
        list(workspaces.iter().map(|p| hit_area(p, config)).collect()).width(Length::Fill);
    let labels = row![
        ticks,
        row![keys, names],
        indicated(urgency.into(), workspaces, config)
    ]
    .spacing(config.spacing.widget_spacing_px);
    Some(stack![labels, hit_areas].into())
}

fn indicated<'a>(
    rows: Element<'a, WorkspacesMessage>,
    workspaces: &[niri::WsPill],
    config: &Config,
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
    focus_indicator(rows, active, color)
        .thickness(config.widgets.indicator_thickness_px)
        .edge(Position::Left)
        .into()
}

fn hit_area<'a>(p: &niri::WsPill, config: &Config) -> Element<'a, WorkspacesMessage> {
    let pressable = button(
        Space::new()
            .width(Length::Fill)
            .height(config.chrome.outer_height_px),
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
}

fn row_cell<'a>(
    content: impl Into<Element<'a, WorkspacesMessage>>,
    config: &Config,
) -> Element<'a, WorkspacesMessage> {
    container(content)
        .height(config.chrome.outer_height_px)
        .align_y(Vertical::Center)
        .into()
}

fn window_ticks<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
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
        edge_activity_line(Position::Left, color, 1.0, thickness)
    });
    container(Column::with_children(segments).spacing(1.0))
        .width(thickness)
        .height(config.chrome.outer_height_px)
        .into()
}

fn urgency_line<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    let opacity = if p.is_urgent { 1.0 } else { 0.0 };
    container(edge_activity_line(
        Position::Left,
        config.palette.urgent,
        opacity,
        config.widgets.indicator_thickness_px,
    ))
    .height(config.chrome.outer_height_px)
    .into()
}

fn index_key(p: &niri::WsPill) -> String {
    if p.idx == 10 {
        "0".to_string()
    } else {
        p.idx.to_string()
    }
}

fn given_name(p: &niri::WsPill) -> Option<&str> {
    p.name.as_deref().filter(|name| !name.is_empty())
}

fn superscript<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    if given_name(p).is_none() || p.idx > 10 {
        return row_cell(Space::new(), config);
    }
    let name_line = LINE_HEIGHT.to_absolute(config.font.text_size_px.into()).0;
    row_cell(
        container(
            bar_text(&config.font, index_key(p))
                .size(config.font.text_size_px * SUPERSCRIPT_SCALE)
                .color(config.palette.inactive),
        )
        .height(name_line)
        .align_y(Vertical::Top),
        config,
    )
}

fn name<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    let label = given_name(p).map_or_else(|| index_key(p), str::to_string);
    row_cell(bar_text(&config.font, label), config)
}
