use std::{iter::once, mem::take};

use corredo::{
    drag_reorder::ListEndCorners,
    orientation::Axis,
    popovers::{NestedMenuRowEmphasis, NestedMenuRowLook},
    theming::{background_strong_color, background_weak_color},
    transitions::ExpansionState,
    width_layout::equal_width_column,
};
use iced::{
    Border, Element, Length, Theme,
    border::Radius,
    widget::{container, rule, stack},
};

use crate::{
    group_menus::{
        group_menu_view::{GroupMenuCascadeSurfaces, InspectionCard},
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, group_row, item_row},
            menu_row::menu_row_height,
        },
    },
    hover_cards::PopupMessage,
    widget_groups::widget_group_content::{GroupEvent, GroupMenuItemRow, RowInspection},
    widgets::configured_scroll::configured_scroll,
};

const SECTION_DIVIDER_PX: f32 = 1.0;

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

fn rows_above_section(sections: &[AccordionSectionRows], section: usize) -> usize {
    sections[..section]
        .iter()
        .map(|earlier_section| earlier_section.shown_rows())
        .sum()
}

fn rows_above_item(sections: &[AccordionSectionRows], section: usize, item: usize) -> usize {
    rows_above_section(sections, section) + 1 + item
}

fn section_row<'a, Message: 'a>(
    row: Element<'a, Message>,
    expansion_state: ExpansionState,
    row_corners: Radius,
    divider: SectionDivider,
) -> Element<'a, Message> {
    let backed = container(row)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            background: match expansion_state {
                ExpansionState::Expanded => Some(background_weak_color(theme).into()),
                ExpansionState::Collapsed => None,
            },
            border: Border {
                radius: row_corners,
                ..Border::default()
            },
            ..container::Style::default()
        });
    match divider {
        SectionDivider::Above => stack![
            backed,
            container(
                rule::horizontal(SECTION_DIVIDER_PX).style(|theme: &Theme| rule::Style {
                    color: background_strong_color(theme),
                    radius: Radius::default(),
                    fill_mode: rule::FillMode::Full,
                    snap: true,
                })
            )
            .align_top(Length::Fill)
        ]
        .into(),
        SectionDivider::Absent => backed.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionDivider {
    Above,
    Absent,
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
    let row_corners = ListEndCorners::new(
        sections.iter().map(|section| section.shown_rows()).sum(),
        frame_corners,
    );
    let inspected_target = layout.group_menus.inspected_target(topic);
    let scrolled_px = layout.group_menus.scrolled_px(topic);
    let mut beside_card = None;
    let section_rows: Vec<Element<'a, M>> = entries
        .into_iter()
        .enumerate()
        .flat_map(|(section, mut entry)| {
            let expansion_state = sections[section].expansion_state;
            let group = entry.group_identity.clone();
            let items: Vec<GroupMenuItemRow<'a, M>> = match expansion_state {
                ExpansionState::Expanded => take(&mut entry.items),
                ExpansionState::Collapsed => Vec::new(),
            };
            let header_corners = row_corners.radius_at(rows_above_section(&sections, section));
            let header_look = NestedMenuRowLook {
                emphasis: match expansion_state {
                    ExpansionState::Expanded => NestedMenuRowEmphasis::Opened,
                    ExpansionState::Collapsed => NestedMenuRowEmphasis::Plain,
                },
                frame_corners: header_corners,
            };
            let divider = match section {
                0 => SectionDivider::Absent,
                _ => SectionDivider::Above,
            };
            let header = section_row(
                group_row(layout, entry, header_look).map(M::from),
                expansion_state,
                header_corners,
                divider,
            );
            let item_views = items.into_iter().enumerate().map(|(item, row)| {
                let rows_above = rows_above_item(&sections, section, item);
                let row_inspection = row.inspection(inspected_target);
                let look = NestedMenuRowLook {
                    emphasis: NestedMenuRowEmphasis::from(row_inspection),
                    frame_corners: row_corners.radius_at(rows_above),
                };
                let row_view = section_row(
                    item_row(layout, &group, row, look).map(M::from),
                    expansion_state,
                    row_corners.radius_at(rows_above),
                    SectionDivider::Absent,
                );
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
            once(header).chain(item_views).collect::<Vec<_>>()
        })
        .collect();
    let scroll_message = layout.menu.scroll_message();
    let rows_scroller = configured_scroll(
        equal_width_column(section_rows),
        Axis::Vertical,
        layout.config,
    )
    .edge_fade(
        layout
            .cascade_satellite_panels
            .rows_edge_fade(layout.config),
    )
    .controlled(scrolled_px, move |scrolled_px| {
        M::from(scroll_message(scrolled_px))
    });
    let primary = rows_scroller.into();
    Some(GroupMenuCascadeSurfaces {
        primary,
        satellites: card_beside_inspected_row(layout, beside_card, scrolled_px),
    })
}

fn card_beside_inspected_row<'a, M: Clone + From<GroupEvent<M>> + From<PopupMessage> + 'a>(
    layout: &GroupMenuRowContext<'_, 'a, M>,
    beside_card: Option<InspectedAccordionRow>,
    scrolled_px: f32,
) -> Vec<Element<'a, M>> {
    beside_card
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AccordionSectionRows, ExpansionState, rows_above_item};

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
    fn items_follow_their_header() {
        let sections = [CLOSED_FOUR, OPEN_THREE];
        assert_eq!(rows_above_item(&sections, 1, 0), 2);
        assert_eq!(rows_above_item(&sections, 1, 2), 4);
    }
}
