use iced::{
    Border, Element, Length, Theme,
    border::Radius,
    widget::{container, rule, stack},
};

use super::{
    MenuBuilder, MenuEntry, MenuItem, MenuSubmenu, NestedMenuSurfaces,
    cascading_panels::BesideCard,
    menu_entries::{BesideRow, icon_column_of, without_redundant_dividers},
};
use crate::{
    drag_reorder::ListEndCorners,
    popovers::{
        MenuRowContent,
        menu_row::{SatelliteReach, gestured_menu_row},
    },
    theming::{background_strong_color, background_weak_color, style_metrics::OUTLINE_WIDTH_PX},
    transitions::ExpansionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SectionDivider {
    Above,
    Absent,
}

pub(super) enum AccordionLine<'a, SubmenuKey, Message> {
    Item {
        item: MenuItem<'a, Message>,
        depth: usize,
    },
    Divider {
        depth: usize,
    },
    Header {
        header: MenuRowContent<'a, Message>,
        path: Vec<SubmenuKey>,
        expansion_state: ExpansionState,
        section_divider: SectionDivider,
    },
}

impl<'a, SubmenuKey: Clone + PartialEq + 'a, Message: Clone + 'a>
    MenuBuilder<'a, SubmenuKey, Message>
{
    pub(super) fn accordion_lines(
        &self,
        entries: Vec<MenuEntry<'a, SubmenuKey, Message>>,
        ancestors: &[SubmenuKey],
    ) -> Vec<AccordionLine<'a, SubmenuKey, Message>> {
        let depth = ancestors.len();
        let entries = without_redundant_dividers(entries);
        let section_dividers: Vec<SectionDivider> = [SectionDivider::Absent]
            .into_iter()
            .chain(entries.iter().map(|entry| match (depth, entry) {
                (0, MenuEntry::Submenu(_)) => SectionDivider::Above,
                _ => SectionDivider::Absent,
            }))
            .collect();
        entries
            .into_iter()
            .zip(section_dividers)
            .flat_map(|(entry, section_divider)| match entry {
                MenuEntry::Item(item) => vec![AccordionLine::Item { item, depth }],
                MenuEntry::Divider => vec![AccordionLine::Divider { depth }],
                MenuEntry::Submenu(MenuSubmenu {
                    key,
                    header,
                    entries,
                }) => {
                    let path: Vec<SubmenuKey> =
                        ancestors.iter().cloned().chain([key.clone()]).collect();
                    let expansion_state = self.expansion_of(&key);
                    let shown_lines = match expansion_state {
                        ExpansionState::Expanded => self.accordion_lines(entries, &path),
                        ExpansionState::Collapsed => Vec::new(),
                    };
                    [AccordionLine::Header {
                        header,
                        path,
                        expansion_state,
                        section_divider,
                    }]
                    .into_iter()
                    .chain(shown_lines)
                    .collect()
                }
            })
            .collect()
    }

    pub(super) fn accordion_line_height_px(
        &self,
        line: &AccordionLine<'a, SubmenuKey, Message>,
    ) -> f32 {
        match line {
            AccordionLine::Divider { .. } => self.divider_height_px(),
            AccordionLine::Item { .. } | AccordionLine::Header { .. } => {
                self.chrome.metrics.row_height_px
            }
        }
    }

    pub(super) fn accordion_surfaces(
        &self,
        entries: Vec<MenuEntry<'a, SubmenuKey, Message>>,
    ) -> NestedMenuSurfaces<'a, Message> {
        let lines = self.accordion_lines(entries, &[]);
        let icon_column = icon_column_of(lines.iter().filter_map(|line| match line {
            AccordionLine::Item { item, .. } => Some(&item.row.content),
            AccordionLine::Header { header, .. } => Some(header),
            AccordionLine::Divider { .. } => None,
        }));
        let corners = ListEndCorners::new(lines.len(), self.chrome.style.frame_corners());
        let row_offsets: Vec<f32> = lines
            .iter()
            .scan(0.0, |next_offset_px, line| {
                let row_offset_px = *next_offset_px;
                *next_offset_px += self.accordion_line_height_px(line);
                Some(row_offset_px)
            })
            .collect();
        let mut beside_card = BesideCard::Absent;
        let rows = lines
            .into_iter()
            .zip(row_offsets)
            .enumerate()
            .map(|(position, (line, row_offset_px))| {
                let row_corners = corners.radius_at(position);
                match line {
                    AccordionLine::Divider { depth } => section_row(
                        self.divider(),
                        section_backing(depth),
                        row_corners,
                        SectionDivider::Absent,
                    ),
                    AccordionLine::Item {
                        item:
                            MenuItem {
                                mut row,
                                beside,
                                wrapping,
                            },
                        depth,
                    } => {
                        if let BesideRow::Card(card) = beside {
                            beside_card = BesideCard::Card {
                                row_offset_px,
                                card,
                            };
                        }
                        row.content = row.content.within(icon_column);
                        row.content.indent_levels = depth;
                        section_row(
                            wrapping.applied(gestured_menu_row(
                                row,
                                self.chrome.metrics,
                                row_corners,
                            )),
                            section_backing(depth),
                            row_corners,
                            SectionDivider::Absent,
                        )
                    }
                    AccordionLine::Header {
                        mut header,
                        path,
                        expansion_state,
                        section_divider,
                    } => {
                        let depth = path.len() - 1;
                        header = header.within(icon_column);
                        header.indent_levels = depth;
                        let backing = match expansion_state {
                            ExpansionState::Expanded => ExpansionState::Expanded,
                            ExpansionState::Collapsed => section_backing(depth),
                        };
                        section_row(
                            gestured_menu_row(
                                self.header_row(
                                    header,
                                    &path,
                                    expansion_state,
                                    SatelliteReach::Closed,
                                ),
                                self.chrome.metrics,
                                row_corners,
                            ),
                            backing,
                            row_corners,
                            section_divider,
                        )
                    }
                }
            })
            .collect();
        let scrolled_px = self.state.scrolled_px(0);
        NestedMenuSurfaces {
            primary: self.scrolled_list(rows, 0),
            satellites: self.primary_beside_card(beside_card, scrolled_px),
        }
    }
}

fn section_backing(depth: usize) -> ExpansionState {
    match depth {
        0 => ExpansionState::Collapsed,
        _ => ExpansionState::Expanded,
    }
}

fn section_row<'a, Message: 'a>(
    row: Element<'a, Message>,
    backing: ExpansionState,
    row_corners: Radius,
    divider: SectionDivider,
) -> Element<'a, Message> {
    let backed = container(row)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            background: match backing {
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
                rule::horizontal(OUTLINE_WIDTH_PX).style(|theme: &Theme| rule::Style {
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
