use std::{iter::once, mem::take};

use corredo::{
    orientation::Axis,
    popovers::{NestedMenuRowEmphasis, NestedMenuRowLook, PopupGrowth},
    selection_scroll::selection_following_scroll,
    transitions::ExpansionState,
    width_layout::equal_width_column,
};
use iced::Element;

use crate::{
    group_menus::{
        accordion_layout::pointer_held_height::pointer_held_height,
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, group_row, item_row},
            menu_row::menu_row_height,
        },
    },
    hover_cards::PopupMessage,
    widget_groups::widget_group_content::{GroupEvent, GroupMenuItemRow, RowInspection},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccordionSectionRows {
    item_count: usize,
    expansion_state: ExpansionState,
}

impl AccordionSectionRows {
    const fn shown_rows(self) -> usize {
        match self.expansion_state {
            ExpansionState::Expanded => self.item_count + 1,
            ExpansionState::Collapsed => 1,
        }
    }
}

fn rows_above_item(
    sections: &[AccordionSectionRows],
    section: usize,
    item: usize,
    growth: PopupGrowth,
) -> usize {
    let earlier: usize = sections[..section]
        .iter()
        .map(|earlier_section| earlier_section.shown_rows())
        .sum();
    match growth {
        PopupGrowth::Downward => earlier + 1 + item,
        PopupGrowth::Upward => earlier + item,
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
            expansion_state: if open_groups.contains(&entry.group_identity) {
                ExpansionState::Expanded
            } else {
                ExpansionState::Collapsed
            },
        })
        .collect();
    let frame_corners = layout.cascade_satellite_panels.frame_corners;
    let inspected_target = layout.group_menus.inspected_target(topic);
    let scrolled_px = layout.group_menus.scrolled_px(topic);
    let mut beside_card = None;
    let section_views: Vec<Element<'a, M>> = entries
        .into_iter()
        .enumerate()
        .map(|(section, mut entry)| {
            let expansion_state = sections[section].expansion_state;
            let group = entry.group_identity.clone();
            let items: Vec<GroupMenuItemRow<'a, M>> = match expansion_state {
                ExpansionState::Expanded => take(&mut entry.items),
                ExpansionState::Collapsed => Vec::new(),
            };
            let header_look = NestedMenuRowLook {
                emphasis: match expansion_state {
                    ExpansionState::Expanded => NestedMenuRowEmphasis::Selected,
                    ExpansionState::Collapsed => NestedMenuRowEmphasis::Plain,
                },
                frame_corners,
            };
            let header: Element<'a, M> = group_row(layout, entry, header_look).map(M::from);
            let item_views = items.into_iter().enumerate().map(|(item, row)| {
                let rows_above = rows_above_item(&sections, section, item, growth);
                let row_inspection = row.inspection(inspected_target);
                let look = NestedMenuRowLook {
                    emphasis: NestedMenuRowEmphasis::from(row_inspection),
                    frame_corners,
                };
                let row_view: Element<'a, M> = item_row(layout, &group, row, look).map(M::from);
                let card_view = (row_inspection == RowInspection::Inspected)
                    .then(|| card.view(layout.config))
                    .flatten();
                if let Some(card_view) = card_view {
                    beside_card = Some(InspectedAccordionRow {
                        rows_above,
                        card_view,
                    });
                }
                row_view
            });
            let rows: Vec<Element<'a, M>> = match growth {
                PopupGrowth::Downward => once(header).chain(item_views).collect(),
                PopupGrowth::Upward => item_views.chain(once(header)).collect(),
            };
            equal_width_column(rows).into()
        })
        .collect();
    let scroll_message = layout.menu.scroll_message();
    let rows_scroller =
        selection_following_scroll(equal_width_column(section_views), Axis::Vertical)
            .edge_fade(
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

#[cfg(test)]
mod tests {
    use super::{AccordionSectionRows, ExpansionState, PopupGrowth, rows_above_item};

    const OPEN_THREE: AccordionSectionRows = AccordionSectionRows {
        item_count: 3,
        expansion_state: ExpansionState::Expanded,
    };
    const CLOSED_FOUR: AccordionSectionRows = AccordionSectionRows {
        item_count: 4,
        expansion_state: ExpansionState::Collapsed,
    };

    #[test]
    fn a_closed_section_counts_only_its_header() {
        assert_eq!(CLOSED_FOUR.shown_rows(), 1);
        assert_eq!(OPEN_THREE.shown_rows(), 4);
    }

    #[test]
    fn items_follow_their_header_when_growing_downward() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_item(&sections, 1, 0, PopupGrowth::Downward), 2);
        assert_eq!(rows_above_item(&sections, 1, 2, PopupGrowth::Downward), 4);
    }

    #[test]
    fn items_sit_above_their_header_when_growing_upward() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_item(&sections, 1, 0, PopupGrowth::Upward), 1);
        assert_eq!(rows_above_item(&sections, 1, 2, PopupGrowth::Upward), 3);
    }
}
