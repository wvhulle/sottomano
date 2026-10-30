use iced::{
    Alignment, Color, Element, Length, Theme,
    border::Radius,
    widget::{Row, Text, container, row, text},
};
use sottomano_config::{Config, screen_edges::Position};
use sottomano_sources::desktop_windows_apps::{
    niri_kdl_config::ConfiguredWorkspaces, niri_window_snapshot,
};

use super::{
    WorkspacesMessage, dragged, order_restore,
    workspace_slot::{
        SlotTone, drop_tint, edge_line, hit_area, indicated, slot_tone, window_markers,
    },
};
use crate::{
    clock::Clock,
    widgets::{
        configured_theme::{secondary_text_color, text_color},
        pager_slot::PagerSlot,
        pointer_drag::{
            BarSlotList, DragView, DraggedBarSlot, draggable_slot::draggable_slot_list,
            keyed_slot_list::keyed_slot_list,
        },
        text::bar_text,
    },
};

pub fn inline_view<'a>(
    niri: &niri_window_snapshot::Snapshot,
    output: &str,
    config: &Config,
    position: Position,
    drag: &DragView,
    configured: Option<&ConfiguredWorkspaces>,
    clock: Clock,
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
            edge_line(config, p, position, tone, drag, clock),
        )
        .ticks(window_markers(config, niri, p.id, position, drag, tone).view())
        .spaced_by(config.layout.panel.widget_spacing)
        .view(config, position);
        let tinted = container(slot)
            .style(drop_tint(p, drag, Radius::default()))
            .into();
        (
            DraggedBarSlot::Workspace(p.id),
            hit_area(p, position_in_bar, tinted, drag),
        )
    });
    let list = keyed_slot_list(slots, position, config).into();
    let draggable = draggable_slot_list(
        indicated(list, &order, config, position, Radius::default()),
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

pub(super) fn given_name(p: &niri_window_snapshot::WorkspaceIndicator) -> Option<&str> {
    p.name.as_deref().filter(|name| !name.is_empty())
}

pub(super) fn display_name(p: &niri_window_snapshot::WorkspaceIndicator) -> String {
    given_name(p).map_or_else(|| p.idx.to_string(), str::to_string)
}

pub(super) fn has_superscript(p: &niri_window_snapshot::WorkspaceIndicator) -> bool {
    given_name(p).is_some() && p.idx <= 9
}

fn tinted_text(color: impl Fn(&Theme) -> Color + Copy + 'static) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(theme)),
    }
}

pub(super) fn superscript<'a>(
    config: &Config,
    p: &niri_window_snapshot::WorkspaceIndicator,
    tone: SlotTone,
) -> Text<'a> {
    bar_text(&config.theme.font, p.idx.to_string())
        .size(config.theme.font.small_text_size_px)
        .style(tinted_text(tone.tint(secondary_text_color)))
}

pub(super) fn inline_label<'a>(
    config: &Config,
    p: &niri_window_snapshot::WorkspaceIndicator,
    tone: SlotTone,
) -> Element<'a, WorkspacesMessage> {
    let name =
        bar_text(&config.theme.font, display_name(p)).style(tinted_text(tone.tint(text_color)));
    if !has_superscript(p) {
        return name.into();
    }
    row![superscript(config, p, tone), name]
        .align_y(Alignment::Start)
        .into()
}
