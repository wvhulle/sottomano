use iced::{
    Color, Element, Theme, border,
    widget::{container, progress_bar},
};
use sottomano_config::{Config, screen_edges::Position};
use sottomano_sources::desktop_windows_apps::niri_window_snapshot;

use super::{DROP_TARGET_ALPHA, WorkspacesMessage, dragged};
use crate::{
    bar_slot_drag::{SlotClick, SlotPress},
    clock::Clock,
    hover_tooltips::{HoverTarget, PressHints, hoverable},
    widgets::{
        activity_line::edge_activity_line,
        focus_indicator::focus_indicator,
        marker_track::MarkerTrack,
        palette_colors::PaletteColors,
        pointer_drag::{DragView, DraggedBarSlot, draggable_slot::draggable_slot},
    },
};

pub(super) fn drop_tint(
    config: &Config,
    p: &niri_window_snapshot::WorkspaceIndicator,
    drag: &DragView,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let tint = drag.awaits_window_on(p.id).then(|| {
        PaletteColors::from(&config.theme.palette)
            .focus
            .scale_alpha(DROP_TARGET_ALPHA)
    });
    move |_| tint.map_or_else(container::Style::default, container::background)
}

pub(super) fn edge_line<'a>(
    config: &Config,
    p: &niri_window_snapshot::WorkspaceIndicator,
    position: Position,
    tone: SlotTone,
    drag: &DragView,
    clock: Clock,
) -> Element<'a, WorkspacesMessage> {
    let Some(charge) = drag.hover_charge(
        DraggedBarSlot::Workspace(p.id),
        config.panel_bar.drag_spring_load,
        clock.instant,
    ) else {
        return urgency_line(config, p, position, tone);
    };
    let focus = PaletteColors::from(&config.theme.palette).focus;
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
pub(super) enum SlotTone {
    Regular,
    Dragged,
}

impl SlotTone {
    pub(super) fn tint(self, config: &Config, regular: Color) -> Color {
        match self {
            SlotTone::Regular => regular,
            SlotTone::Dragged => PaletteColors::from(&config.theme.palette).dim,
        }
    }
}

pub(super) fn slot_tone(p: &niri_window_snapshot::WorkspaceIndicator, drag: &DragView) -> SlotTone {
    if drag.is_dragging(DraggedBarSlot::Workspace(p.id)) {
        SlotTone::Dragged
    } else {
        SlotTone::Regular
    }
}

pub(super) fn indicated<'a>(
    slots: Element<'a, WorkspacesMessage>,
    workspaces: &[&niri_window_snapshot::WorkspaceIndicator],
    config: &Config,
    position: Position,
) -> Element<'a, WorkspacesMessage> {
    let active = workspaces.iter().position(|p| p.is_active);
    let active_is_urgent = workspaces.iter().any(|p| p.is_active && p.is_urgent);
    let color = if active_is_urgent {
        PaletteColors::from(&config.theme.palette).urgent
    } else {
        PaletteColors::from(&config.theme.palette).focus
    };
    focus_indicator(slots, active, color, config)
        .edge(position)
        .into()
}

pub(super) fn hit_area<'a>(
    p: &niri_window_snapshot::WorkspaceIndicator,
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

pub(super) fn window_color(
    config: &Config,
    w: &niri_window_snapshot::WinEntry,
    unfocused: Color,
) -> Color {
    if w.is_urgent {
        PaletteColors::from(&config.theme.palette).urgent
    } else if w.is_focused {
        PaletteColors::from(&config.theme.palette).focus
    } else {
        unfocused
    }
}

pub(super) fn window_markers(
    config: &Config,
    niri: &niri_window_snapshot::Snapshot,
    workspace: u64,
    position: Position,
    drag: &DragView,
    tone: SlotTone,
) -> MarkerTrack {
    let incoming = drag
        .awaits_window_on(workspace)
        .then_some(PaletteColors::from(&config.theme.palette).focus);
    let colors = niri
        .windows_on_workspace(workspace)
        .iter()
        .map(|w| {
            tone.tint(
                config,
                window_color(
                    config,
                    w,
                    PaletteColors::from(&config.theme.palette).inactive,
                ),
            )
        })
        .chain(incoming)
        .collect();
    MarkerTrack::new(colors, config.panel_bar.indicator_thickness, position)
}

pub(super) fn urgency_line<'a>(
    config: &Config,
    p: &niri_window_snapshot::WorkspaceIndicator,
    position: Position,
    tone: SlotTone,
) -> Element<'a, WorkspacesMessage> {
    let opacity = if p.is_urgent { 1.0 } else { 0.0 };
    edge_activity_line(
        position,
        tone.tint(config, PaletteColors::from(&config.theme.palette).urgent),
        opacity,
        config.panel_bar.indicator_thickness,
    )
}
