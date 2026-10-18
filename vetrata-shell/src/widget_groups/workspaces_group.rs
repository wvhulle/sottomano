pub mod workspace_preview;
pub mod workspace_tooltip;

use iced::{
    Alignment, Color, Element, Length, Theme, border,
    widget::{Column, Row, Text, column, container, progress_bar, row},
};

use crate::{
    config::Config,
    data::source::{niri, niri_config::ConfiguredWorkspaces},
    desktop_shell::{
        Message,
        hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
        monitor_surfaces::Position,
        pointer_drag::{DragMessage, SlotClick, SlotDragEvent, SlotPress},
    },
    widgets::{
        activity_line::edge_activity_line,
        focus_indicator::focus_indicator,
        glyph_button,
        marker_track::MarkerTrack,
        pager_slot::PagerSlot,
        panel_frame::panel_frame,
        phosphor::{Phosphor, Weight},
        pointer_drag::{
            BarSlotList, DragView, DraggedBarSlot,
            draggable_slot::{draggable_slot, draggable_slot_list},
            keyed_slot_list::keyed_slot_list,
        },
        text::bar_text,
    },
};

const DROP_TARGET_ALPHA: f32 = 0.4;

#[derive(Debug, Clone)]
pub enum WorkspacesMessage {
    Tooltip(TooltipMessage),
    Drag(DragMessage),
    RestoreOrder { output: String },
}

impl From<WorkspacesMessage> for Message {
    fn from(message: WorkspacesMessage) -> Self {
        match message {
            WorkspacesMessage::Tooltip(tooltip) => Message::Tooltip(tooltip),
            WorkspacesMessage::Drag(drag) => Message::PointerDrag(drag),
            WorkspacesMessage::RestoreOrder { output } => Message::RestoreWorkspaceOrder { output },
        }
    }
}

fn order_restore<'a>(
    configured: Option<&ConfiguredWorkspaces>,
    workspaces: &[niri::WorkspaceIndicator],
    output: &str,
    config: &Config,
) -> Option<Element<'a, WorkspacesMessage>> {
    configured.filter(|configured| !configured.is_followed_on(workspaces))?;
    Some(hoverable(
        glyph_button(
            Phosphor::ArrowCounterClockwise(Weight::Regular),
            config.theme.font.icon_size_px,
            config.theme.palette.inactive,
            WorkspacesMessage::RestoreOrder {
                output: output.to_owned(),
            },
        ),
        HoverTarget::WorkspaceOrder,
        PressHints {
            left: Some("restore the order from niri's config"),
            ..PressHints::default()
        },
        WorkspacesMessage::Tooltip,
    ))
}

pub fn vertical_view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
    drag: &DragView,
    configured: Option<&ConfiguredWorkspaces>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let order = drag.previewed(workspaces, |p| DraggedBarSlot::Workspace(p.id));
    let rows = order.iter().enumerate().map(|(position, p)| {
        let row = sidebar_row(config, niri, p, drag);
        (
            DraggedBarSlot::Workspace(p.id),
            hit_area(p, position, row, drag),
        )
    });
    let list = keyed_slot_list(rows, Position::Left, config)
        .spacing(config.layout.panel.widget_spacing)
        .into();
    let draggable = draggable_slot_list(
        indicated(list, &order, config, Position::Left),
        BarSlotList::Workspaces,
        drag,
        dragged,
    );
    Some(
        Column::new()
            .extend(order_restore(configured, workspaces, output, config))
            .push(draggable)
            .spacing(config.layout.panel.widget_spacing)
            .align_x(Alignment::Center)
            .into(),
    )
}

#[must_use]
pub fn drag_ghost<'a>(
    niri: &niri::Snapshot,
    output: &str,
    workspace: u64,
    config: &Config,
    drag: &DragView,
) -> Option<Element<'a, WorkspacesMessage>> {
    let p = niri
        .workspaces_for_output(output)
        .iter()
        .find(|p| p.id == workspace)?;
    Some(
        panel_frame(
            container(inline_label(config, p, SlotTone::Regular))
                .padding([0.0, config.layout.panel.group_padding])
                .center_y(config.layout.panel.height),
            &drag.ghost_frame,
        )
        .into(),
    )
}

fn dragged(event: SlotDragEvent) -> WorkspacesMessage {
    WorkspacesMessage::Drag(DragMessage::Pointer(event))
}

fn sidebar_row<'a>(
    config: &Config,
    niri: &niri::Snapshot,
    p: &niri::WorkspaceIndicator,
    drag: &DragView,
) -> Element<'a, WorkspacesMessage> {
    let tone = slot_tone(p, drag);
    let label_over_markers = column![
        inline_label(config, p, tone),
        window_markers(config, niri, p.id, Position::Bottom, drag, tone).view()
    ]
    .width(Length::Shrink)
    .align_x(Alignment::Center);
    let label_with_edge = row![
        label_over_markers,
        edge_line(config, p, Position::Left, tone, drag)
    ]
    .spacing(config.layout.panel.widget_spacing)
    .height(config.layout.panel.height)
    .align_y(Alignment::Center);
    container(label_with_edge)
        .width(Length::Fill)
        .align_x(Alignment::End)
        .style(drop_tint(config, p, drag))
        .into()
}

pub fn inline_view<'a>(
    niri: &niri::Snapshot,
    output: &str,
    config: &Config,
    position: Position,
    drag: &DragView,
    configured: Option<&ConfiguredWorkspaces>,
) -> Option<Element<'a, WorkspacesMessage>> {
    let workspaces = niri.workspaces_for_output(output);
    if workspaces.is_empty() {
        return None;
    }
    let order = drag.previewed(workspaces, |p| DraggedBarSlot::Workspace(p.id));
    let slots = order.iter().enumerate().map(|(position_in_bar, p)| {
        let tone = slot_tone(p, drag);
        let slot = PagerSlot::new(
            inline_label(config, p, tone),
            edge_line(config, p, position, tone, drag),
        )
        .ticks(window_markers(config, niri, p.id, position, drag, tone).view())
        .spaced_by(config.layout.panel.widget_spacing)
        .view(config, position);
        let tinted = container(slot).style(drop_tint(config, p, drag)).into();
        (
            DraggedBarSlot::Workspace(p.id),
            hit_area(p, position_in_bar, tinted, drag),
        )
    });
    let list = keyed_slot_list(slots, position, config).into();
    let draggable = draggable_slot_list(
        indicated(list, &order, config, position),
        BarSlotList::Workspaces,
        drag,
        dragged,
    );
    Some(
        Row::new()
            .push(draggable)
            .extend(order_restore(configured, workspaces, output, config))
            .spacing(config.layout.panel.widget_spacing)
            .height(Length::Fill)
            .align_y(Alignment::Center)
            .into(),
    )
}

fn drop_tint(
    config: &Config,
    p: &niri::WorkspaceIndicator,
    drag: &DragView,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let tint = drag
        .awaits_window_on(p.id)
        .then(|| config.theme.palette.focus.scale_alpha(DROP_TARGET_ALPHA));
    move |_| container::Style {
        background: tint.map(Into::into),
        ..container::Style::default()
    }
}

fn edge_line<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
    position: Position,
    tone: SlotTone,
    drag: &DragView,
) -> Element<'a, WorkspacesMessage> {
    let Some(charge) = drag.hover_charge(
        DraggedBarSlot::Workspace(p.id),
        config.panel_bar.drag_spring_load,
    ) else {
        return urgency_line(config, p, position, tone);
    };
    let focus = config.theme.palette.focus;
    let thickness = config.panel_bar.indicator_thickness;
    let bar = progress_bar(0.0..=1.0, charge)
        .girth(thickness)
        .style(move |_| progress_bar::Style {
            background: focus.scale_alpha(DROP_TARGET_ALPHA).into(),
            bar: focus.into(),
            border: border::rounded(thickness),
        });
    if position.is_vertical() {
        bar.vertical().into()
    } else {
        bar.into()
    }
}

#[derive(Debug, Clone, Copy)]
enum SlotTone {
    Regular,
    Dragged,
}

impl SlotTone {
    fn tint(self, config: &Config, regular: Color) -> Color {
        match self {
            SlotTone::Regular => regular,
            SlotTone::Dragged => config.theme.palette.dim,
        }
    }
}

fn slot_tone(p: &niri::WorkspaceIndicator, drag: &DragView) -> SlotTone {
    if drag.is_dragging(DraggedBarSlot::Workspace(p.id)) {
        SlotTone::Dragged
    } else {
        SlotTone::Regular
    }
}

fn indicated<'a>(
    slots: Element<'a, WorkspacesMessage>,
    workspaces: &[&niri::WorkspaceIndicator],
    config: &Config,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let active = workspaces.iter().position(|p| p.is_active);
    let active_is_urgent = workspaces.iter().any(|p| p.is_active && p.is_urgent);
    let color = if active_is_urgent {
        config.theme.palette.urgent
    } else {
        config.theme.palette.focus
    };
    focus_indicator(slots, active, color, config)
        .edge(position)
        .into()
}

fn hit_area<'a>(
    p: &niri::WorkspaceIndicator,
    position: usize,
    content: Element<'a, WorkspacesMessage>,
    drag: &DragView,
) -> Element<'a, WorkspacesMessage> {
    let press = SlotPress {
        item: DraggedBarSlot::Workspace(p.id),
        position,
        click: SlotClick::Workspace {
            id: p.id,
            is_active: p.is_active,
        },
    };
    let pressable = draggable_slot(content, press, true, drag, dragged);
    hoverable(
        pressable.into(),
        HoverTarget::Workspace(p.id),
        PressHints {
            left: Some(if p.is_active {
                "toggle the overview"
            } else {
                "switch to this workspace"
            }),
            ..PressHints::default()
        },
        WorkspacesMessage::Tooltip,
    )
}

fn window_color(config: &Config, w: &niri::WinEntry, unfocused: Color) -> Color {
    if w.is_urgent {
        config.theme.palette.urgent
    } else if w.is_focused {
        config.theme.palette.focus
    } else {
        unfocused
    }
}

fn window_markers(
    config: &Config,
    niri: &niri::Snapshot,
    workspace: u64,
    position: Position,
    drag: &DragView,
    tone: SlotTone,
) -> MarkerTrack {
    let incoming = drag
        .awaits_window_on(workspace)
        .then_some(config.theme.palette.focus);
    let colors = niri
        .windows_on_workspace(workspace)
        .iter()
        .map(|w| {
            tone.tint(
                config,
                window_color(config, w, config.theme.palette.inactive),
            )
        })
        .chain(incoming)
        .collect();
    MarkerTrack::new(colors, config.panel_bar.indicator_thickness, position)
}

fn urgency_line<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
    position: Position,
    tone: SlotTone,
) -> Element<'a, WorkspacesMessage> {
    let opacity = if p.is_urgent { 1.0 } else { 0.0 };
    edge_activity_line(
        position,
        tone.tint(config, config.theme.palette.urgent),
        opacity,
        config.panel_bar.indicator_thickness,
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

fn superscript<'a>(config: &Config, p: &niri::WorkspaceIndicator, tone: SlotTone) -> Text<'a> {
    bar_text(&config.theme.font, p.idx.to_string())
        .size(config.theme.font.small_text_size_px)
        .color(tone.tint(config, config.theme.palette.inactive))
}

fn inline_label<'a>(
    config: &Config,
    p: &niri::WorkspaceIndicator,
    tone: SlotTone,
) -> Element<'a, WorkspacesMessage> {
    let name = bar_text(&config.theme.font, display_name(p))
        .color(tone.tint(config, config.theme.palette.bar_foreground));
    if !has_superscript(p) {
        return name.into();
    }
    row![superscript(config, p, tone), name]
        .align_y(Alignment::Start)
        .into()
}
