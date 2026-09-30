pub mod workspace_preview;
pub mod workspace_tooltip;

use iced::{
    Alignment, Element, Length,
    alignment::Vertical,
    widget::{Column, Row, Space, Text, button, container, row, stack},
};

use crate::{
    app::Message,
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
        pager_slot::PagerSlot,
        text::{LINE_HEIGHT, bar_text},
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
    let list = |cells: Vec<Element<'a, WorkspacesMessage>>| {
        Column::with_children(cells).spacing(config.spacing.widget_spacing_px)
    };
    let ticks = list(
        workspaces
            .iter()
            .map(|p| {
                container(
                    Column::with_children(tick_segments(config, niri, p.id, Position::Left))
                        .spacing(1.0),
                )
                .width(config.widgets.indicator_thickness_px)
                .height(config.chrome.outer_height_px)
                .into()
            })
            .collect(),
    );
    let keys = list(
        workspaces
            .iter()
            .map(|p| row_cell(superscript_cell(config, p), config))
            .collect(),
    )
    .align_x(Alignment::End);
    let names = list(
        workspaces
            .iter()
            .map(|p| row_cell(bar_text(&config.font, display_name(p)), config))
            .collect(),
    )
    .align_x(Alignment::End);
    let urgency = list(
        workspaces
            .iter()
            .map(|p| {
                container(urgency_line(config, p, Position::Left))
                    .height(config.chrome.outer_height_px)
                    .into()
            })
            .collect(),
    );
    let hit_areas = list(
        workspaces
            .iter()
            .map(|p| {
                hit_area(
                    p,
                    Space::new()
                        .width(Length::Fill)
                        .height(config.chrome.outer_height_px)
                        .into(),
                    config,
                )
            })
            .collect(),
    )
    .width(Length::Fill);
    let labels = row![
        ticks,
        row![keys, names],
        indicated(urgency.into(), workspaces, config, Position::Left)
    ]
    .spacing(config.spacing.widget_spacing_px);
    Some(stack![labels, hit_areas].into())
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
            .ticks(
                Row::with_children(tick_segments(config, niri, p.id, position))
                    .spacing(1.0)
                    .into(),
            )
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

fn hit_area<'a>(
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

fn row_cell<'a>(
    content: impl Into<Element<'a, WorkspacesMessage>>,
    config: &Config,
) -> Element<'a, WorkspacesMessage> {
    container(content)
        .height(config.chrome.outer_height_px)
        .align_y(Vertical::Center)
        .into()
}

fn tick_segments<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
    position: Position,
) -> Vec<Element<'a, WorkspacesMessage>> {
    let thickness = config.widgets.indicator_thickness_px;
    niri.windows_on_workspace(workspace)
        .iter()
        .map(|w| {
            let color = if w.is_urgent {
                config.palette.urgent
            } else if w.is_focused {
                config.palette.focus
            } else {
                config.palette.inactive
            };
            edge_activity_line(position, color, 1.0, thickness)
        })
        .collect()
}

fn urgency_line<'a>(
    config: &Config,
    p: &niri::WsPill,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let opacity = if p.is_urgent { 1.0 } else { 0.0 };
    edge_activity_line(
        position,
        config.palette.urgent,
        opacity,
        config.widgets.indicator_thickness_px,
    )
}

fn given_name(p: &niri::WsPill) -> Option<&str> {
    p.name.as_deref().filter(|name| !name.is_empty())
}

fn display_name(p: &niri::WsPill) -> String {
    given_name(p).map_or_else(|| p.idx.to_string(), str::to_string)
}

fn has_superscript(p: &niri::WsPill) -> bool {
    given_name(p).is_some() && p.idx <= 9
}

fn superscript<'a>(config: &Config, p: &niri::WsPill) -> Text<'a> {
    bar_text(&config.font, p.idx.to_string())
        .size(config.font.text_size_px * SUPERSCRIPT_SCALE)
        .color(config.palette.inactive)
}

fn superscript_cell<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    if !has_superscript(p) {
        return Space::new().into();
    }
    let name_line = LINE_HEIGHT.to_absolute(config.font.text_size_px.into()).0;
    container(superscript(config, p))
        .height(name_line)
        .align_y(Vertical::Top)
        .into()
}

fn inline_label<'a>(config: &Config, p: &niri::WsPill) -> Element<'a, WorkspacesMessage> {
    let name = bar_text(&config.font, display_name(p));
    if !has_superscript(p) {
        return name.into();
    }
    row![superscript(config, p), name]
        .align_y(Alignment::Start)
        .into()
}
