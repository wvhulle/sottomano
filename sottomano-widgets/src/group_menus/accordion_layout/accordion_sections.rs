use std::{iter::once, mem::take};

use iced::Element;

use crate::{
    group_menus::{
        accordion_layout::pointer_held_height::pointer_held_height,
        cascading_layout::notched_panel::PanelGrowth,
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, group_row, item_row},
            menu_row::{MenuRowEmphasis, MenuRowLook, menu_row_height},
        },
    },
    hover_cards::PopupMessage,
    widget_groups::{
        widget_group_content::{GroupEvent, GroupMenuItemRow},
        widget_group_identity::WidgetGroupIdentity,
    },
    widgets::{
        list_end_corners::ListEndCorners,
        selection_scroller::{ScrollAxis, selection_scroller},
        uniform_width_column::uniform_width_column,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccordionSectionRows {
    item_count: usize,
    open: bool,
}

impl AccordionSectionRows {
    const fn shown_rows(self) -> usize {
        if self.open { self.item_count + 1 } else { 1 }
    }
}

fn rows_above_header(
    sections: &[AccordionSectionRows],
    section: usize,
    growth: PanelGrowth,
) -> usize {
    let earlier: usize = sections[..section]
        .iter()
        .map(|earlier_section| earlier_section.shown_rows())
        .sum();
    match growth {
        PanelGrowth::Downward => earlier,
        PanelGrowth::Upward => earlier + sections[section].shown_rows() - 1,
    }
}

fn rows_above_item(
    sections: &[AccordionSectionRows],
    section: usize,
    item: usize,
    growth: PanelGrowth,
) -> usize {
    let earlier: usize = sections[..section]
        .iter()
        .map(|earlier_section| earlier_section.shown_rows())
        .sum();
    match growth {
        PanelGrowth::Downward => earlier + 1 + item,
        PanelGrowth::Upward => earlier + item,
    }
}

struct InspectedAccordionRow {
    rows_above: usize,
    card_view: Element<'static, PopupMessage>,
}

pub(crate) fn accordion_sections<'a, M: Clone + From<GroupEvent<M>> + From<PopupMessage> + 'a>(
    entries: Vec<GroupMenuGroupRow<'a, M>>,
    layout: &GroupMenuRowContext<'_, 'a, M>,
    card: &InspectionCard<'_, 'a>,
) -> Option<GroupMenuCascadeSurfaces<'a, M>> {
    if entries.is_empty() {
        return None;
    }
    let topic = layout.menu.topic;
    let open_groups = layout.group_menus.open_groups(topic);
    let growth = layout.cascade_satellite_panels.growth;
    let sections: Vec<AccordionSectionRows> = entries
        .iter()
        .map(|entry| AccordionSectionRows {
            item_count: entry.items.len(),
            open: is_open(open_groups, &entry.group_identity),
        })
        .collect();
    let total_rows = sections.iter().map(|section| section.shown_rows()).sum();
    let corners = ListEndCorners::new(total_rows, layout.cascade_satellite_panels.frame_corners);
    let inspected_target = layout.group_menus.inspected_target(topic);
    let scrolled_px = layout.group_menus.scrolled_px(topic);
    let mut beside_card = None;
    let section_views: Vec<Element<'a, M>> = entries
        .into_iter()
        .enumerate()
        .map(|(section, mut entry)| {
            let open = sections[section].open;
            let group = entry.group_identity.clone();
            let items: Vec<GroupMenuItemRow<'a, M>> = if open {
                take(&mut entry.items)
            } else {
                Vec::new()
            };
            let header_look = MenuRowLook {
                emphasis: MenuRowEmphasis::selected_when(open),
                corners: corners.radius_at(rows_above_header(&sections, section, growth)),
            };
            let header: Element<'a, M> = group_row(layout, entry, header_look).map(M::from);
            let item_views = items.into_iter().enumerate().map(|(item, row)| {
                let rows_above = rows_above_item(&sections, section, item, growth);
                let inspected = row.is_inspected(inspected_target);
                let look = MenuRowLook {
                    emphasis: MenuRowEmphasis::selected_when(inspected),
                    corners: corners.radius_at(rows_above),
                };
                let row_view: Element<'a, M> = item_row(layout, &group, row, look).map(M::from);
                if let Some(card_view) = inspected.then(|| card.view(layout.config)).flatten() {
                    beside_card = Some(InspectedAccordionRow {
                        rows_above,
                        card_view,
                    });
                }
                row_view
            });
            let rows: Vec<Element<'a, M>> = match growth {
                PanelGrowth::Downward => once(header).chain(item_views).collect(),
                PanelGrowth::Upward => item_views.chain(once(header)).collect(),
            };
            uniform_width_column(rows).into()
        })
        .collect();
    let scroll_message = layout.menu.scroll_message();
    let rows_scroller = selection_scroller(
        uniform_width_column(section_views),
        ScrollAxis::Vertical,
        layout
            .cascade_satellite_panels
            .rows_edge_fade(layout.config),
    )
    .controlled(scrolled_px, move |scrolled_px| {
        M::from(scroll_message(scrolled_px))
    });
    let primary = pointer_held_height(rows_scroller.into(), growth);
    let satellites = beside_card
        .map(|inspected| {
            let row_top =
                inspected.rows_above as f32 * menu_row_height(layout.config) - scrolled_px;
            layout.cascade_satellite_panels.beside_row(
                layout.config,
                inspected.card_view.map(M::from),
                row_top,
            )
        })
        .into_iter()
        .collect();
    Some(GroupMenuCascadeSurfaces {
        primary,
        satellites,
    })
}

fn is_open(open_groups: &[WidgetGroupIdentity], group: &WidgetGroupIdentity) -> bool {
    open_groups.contains(group)
}

#[cfg(test)]
mod tests {
    use super::{AccordionSectionRows, PanelGrowth, rows_above_header, rows_above_item};

    const OPEN_THREE: AccordionSectionRows = AccordionSectionRows {
        item_count: 3,
        open: true,
    };
    const CLOSED_FOUR: AccordionSectionRows = AccordionSectionRows {
        item_count: 4,
        open: false,
    };

    #[test]
    fn a_closed_section_counts_only_its_header() {
        assert_eq!(CLOSED_FOUR.shown_rows(), 1);
        assert_eq!(OPEN_THREE.shown_rows(), 4);
    }

    #[test]
    fn items_follow_their_header_when_growing_downward() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_header(&sections, 1, PanelGrowth::Downward), 1);
        assert_eq!(rows_above_item(&sections, 1, 0, PanelGrowth::Downward), 2);
        assert_eq!(rows_above_item(&sections, 1, 2, PanelGrowth::Downward), 4);
    }

    #[test]
    fn items_sit_above_their_header_when_growing_upward() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_item(&sections, 1, 0, PanelGrowth::Upward), 1);
        assert_eq!(rows_above_item(&sections, 1, 2, PanelGrowth::Upward), 3);
        assert_eq!(rows_above_header(&sections, 1, PanelGrowth::Upward), 4);
    }
}
