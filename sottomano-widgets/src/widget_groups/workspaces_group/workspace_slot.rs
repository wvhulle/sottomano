use corredo::marker_track::{MarkerTrack, TrackAxis};
use iced::{
    Alignment, Border, Color, Element, Length, Theme,
    border::Radius,
    widget::{Text, column, container, progress_bar, row, text},
};
use sottomano_desktop_sources::desktop_windows_apps::workspace_window_snapshot;
use sottomano_toml_config::screen_edges::Position;

use super::{WorkspacesMessage, dragged};
use crate::{
    desktop_panels::{
        panel_slot::PanelSlot,
        panel_slot_drag::{PanelSlotContext, SlotClick, SlotPress, awaits_window_on},
    },
    hover_cards::{HoverTarget, PressHints, hoverable},
    widgets::{
        activity_line::edge_activity_line,
        configured_theme::{
            ThemedColor, background_strong_color, danger_color, primary_color, primary_weak_color,
            secondary_text_color, text_color,
        },
        pointer_drag::{
            DraggedPanelSlot,
            draggable_slot::{HoverReorder, draggable_slot},
        },
        shared_metrics::{ACCENT_STROKE_PX, pill_corners},
        text::panel_text,
    },
};

#[derive(Debug, Clone, Copy)]
enum SlotTone {
    Regular,
    Dragged,
}

impl SlotTone {
    fn tint(
        self,
        regular: impl Fn(&Theme) -> Color + Copy + 'static,
    ) -> impl Fn(&Theme) -> Color + Copy + 'static {
        move |theme| match self {
            SlotTone::Regular => regular(theme),
            SlotTone::Dragged => background_strong_color(theme),
        }
    }
}

fn tinted_text(color: impl Fn(&Theme) -> Color + Copy + 'static) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(theme)),
    }
}

fn window_marker_color(
    window_entry: &workspace_window_snapshot::WinEntry,
    unfocused: fn(&Theme) -> Color,
) -> fn(&Theme) -> Color {
    if window_entry.is_urgent {
        danger_color
    } else if window_entry.is_focused {
        primary_color
    } else {
        unfocused
    }
}

pub(super) fn workspace_slot_key(
    workspace: &workspace_window_snapshot::WorkspaceIndicator,
) -> DraggedPanelSlot {
    DraggedPanelSlot::Workspace(workspace.id)
}

pub(super) fn active_workspace_key(
    order: &[&workspace_window_snapshot::WorkspaceIndicator],
) -> Option<DraggedPanelSlot> {
    order
        .iter()
        .find(|workspace| workspace.is_active)
        .map(|workspace| workspace_slot_key(workspace))
}

pub(super) fn active_highlight_color(
    order: &[&workspace_window_snapshot::WorkspaceIndicator],
) -> fn(&Theme) -> Color {
    if order
        .iter()
        .any(|workspace| workspace.is_active && workspace.is_urgent)
    {
        danger_color
    } else {
        primary_color
    }
}

pub(super) struct WorkspaceSlot<'s> {
    workspace: &'s workspace_window_snapshot::WorkspaceIndicator,
    compositor: &'s workspace_window_snapshot::Snapshot,
    tone: SlotTone,
    position_in_panel: usize,
    slot_context: PanelSlotContext<'s>,
}

impl<'s> WorkspaceSlot<'s> {
    pub(super) fn new(
        workspace: &'s workspace_window_snapshot::WorkspaceIndicator,
        compositor: &'s workspace_window_snapshot::Snapshot,
        position_in_panel: usize,
        slot_context: PanelSlotContext<'s>,
    ) -> Self {
        let slot_key = workspace_slot_key(workspace);
        let tone = match slot_context.drag.drag {
            Some(active_drag) if active_drag.item == slot_key => SlotTone::Dragged,
            Some(_) | None => SlotTone::Regular,
        };
        Self {
            workspace,
            compositor,
            tone,
            position_in_panel,
            slot_context,
        }
    }

    pub(super) fn ghost(
        workspace: &'s workspace_window_snapshot::WorkspaceIndicator,
        compositor: &'s workspace_window_snapshot::Snapshot,
        slot_context: PanelSlotContext<'s>,
    ) -> Self {
        Self {
            tone: SlotTone::Regular,
            ..Self::new(workspace, compositor, 0, slot_context)
        }
    }

    pub(super) fn key(&self) -> DraggedPanelSlot {
        workspace_slot_key(self.workspace)
    }

    fn given_name(&self) -> Option<&str> {
        self.workspace
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
    }

    fn display_name(&self) -> String {
        self.given_name()
            .map_or_else(|| self.workspace.idx.to_string(), str::to_string)
    }

    fn has_superscript(&self) -> bool {
        self.given_name().is_some() && self.workspace.idx <= 9
    }

    fn superscript<'a>(&self) -> Text<'a> {
        let font = &self.slot_context.config.theme.font;
        panel_text(font, self.workspace.idx.to_string())
            .size(font.small_text_size_px)
            .style(tinted_text(self.tone.tint(secondary_text_color)))
    }

    pub(super) fn label<'a>(&self) -> Element<'a, WorkspacesMessage> {
        let name = panel_text(&self.slot_context.config.theme.font, self.display_name())
            .style(tinted_text(self.tone.tint(text_color)));
        if !self.has_superscript() {
            return name.into();
        }
        row![self.superscript(), name]
            .align_y(Alignment::Start)
            .into()
    }

    fn drop_tint(&self, highlight_radius: Radius) -> impl Fn(&Theme) -> container::Style + 'static {
        let awaits_window = awaits_window_on(&self.slot_context.drag, self.workspace.id);
        move |theme| {
            if awaits_window {
                container::background(primary_weak_color(theme)).border(Border {
                    radius: highlight_radius,
                    ..Border::default()
                })
            } else {
                container::Style::default()
            }
        }
    }

    fn edge_line<'a>(&self) -> Element<'a, WorkspacesMessage> {
        let PanelSlotContext {
            config,
            edge,
            drag,
            clock,
            ..
        } = self.slot_context;
        let Some(charge) = drag.hover_charge(
            self.key(),
            config.desktop_panels.slot_drag.spring_load_delay,
            clock.instant,
        ) else {
            return self.urgency_line();
        };
        let thickness = ACCENT_STROKE_PX;
        let charge_bar = progress_bar(0.0..=1.0, charge)
            .girth(thickness)
            .style(move |theme| progress_bar::Style {
                background: primary_weak_color(theme).into(),
                bar: primary_color(theme).into(),
                border: pill_corners(thickness),
            });
        if edge == Position::Left {
            charge_bar.vertical().into()
        } else {
            charge_bar.into()
        }
    }

    fn urgency_line<'a>(&self) -> Element<'a, WorkspacesMessage> {
        let opacity = if self.workspace.is_urgent { 1.0 } else { 0.0 };
        edge_activity_line(
            self.slot_context.edge,
            self.tone.tint(danger_color),
            opacity,
            ACCENT_STROKE_PX,
        )
    }

    fn window_markers(&self, marker_edge: Position) -> MarkerTrack {
        let incoming = awaits_window_on(&self.slot_context.drag, self.workspace.id)
            .then(|| -> ThemedColor<'static> { Box::new(primary_color) });
        let tone = self.tone;
        let colors = self
            .compositor
            .windows_on_workspace(self.workspace.id)
            .iter()
            .map(|window_entry| -> ThemedColor<'static> {
                Box::new(tone.tint(window_marker_color(window_entry, secondary_text_color)))
            })
            .chain(incoming)
            .collect();
        MarkerTrack::new(
            colors,
            ACCENT_STROKE_PX,
            if marker_edge == Position::Left {
                TrackAxis::Vertical
            } else {
                TrackAxis::Horizontal
            },
        )
    }

    fn hit_area<'a>(
        &self,
        content: Element<'a, WorkspacesMessage>,
    ) -> Element<'a, WorkspacesMessage> {
        let press = SlotPress {
            item: self.key(),
            position: self.position_in_panel,
            click: SlotClick::Workspace {
                id: self.workspace.id,
                is_active: self.workspace.is_active,
            },
        };
        let pressable = draggable_slot(
            content,
            press,
            HoverReorder::ReordersOnHover,
            &self.slot_context.drag,
            dragged,
        );
        hoverable(
            pressable.into(),
            HoverTarget::Workspace(self.workspace.id),
            PressHints {
                left: Some(if self.workspace.is_active {
                    "toggle the overview"
                } else {
                    "switch to this workspace"
                }),
                ..PressHints::default()
            },
            WorkspacesMessage::HoverCard,
        )
    }

    pub(super) fn inline_view<'a>(&self) -> Element<'a, WorkspacesMessage> {
        let PanelSlotContext { config, edge, .. } = self.slot_context;
        let panel_slot = PanelSlot::new(self.label(), self.edge_line())
            .ticks(self.window_markers(edge).view())
            .padded_along_panel(config.theme.spacing.item_gap)
            .view(edge);
        self.hit_area(
            container(panel_slot)
                .style(self.drop_tint(Radius::default()))
                .into(),
        )
    }

    pub(super) fn sidebar_view<'a>(
        &self,
        highlight_radius: Radius,
    ) -> Element<'a, WorkspacesMessage> {
        let config = self.slot_context.config;
        let label_over_markers =
            column![self.label(), self.window_markers(Position::Bottom).view()]
                .width(Length::Shrink)
                .align_x(Alignment::Center);
        let label_with_edge = row![label_over_markers, self.edge_line()]
            .spacing(config.theme.spacing.item_gap)
            .height(config.desktop_panels.height)
            .align_y(Alignment::Center);
        self.hit_area(
            container(label_with_edge)
                .align_right(Length::Fill)
                .style(self.drop_tint(highlight_radius))
                .into(),
        )
    }
}
