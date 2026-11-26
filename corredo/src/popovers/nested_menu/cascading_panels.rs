use iced::Element;

use super::{
    MenuBuilder, MenuEntry, MenuItem, MenuSubmenu, NestedMenuSurfaces,
    menu_entries::{BesideRow, shared_icon_column, without_redundant_dividers},
    submenu_header::SubmenuContents,
};
use crate::{
    drag_reorder::ListEndCorners,
    popovers::{
        CascadeDirection, NotchRowAnchor, NotchedPanel, SatelliteRole,
        menu_row::{IconColumn, IconSide, SatelliteReach, gestured_menu_row},
    },
    transitions::ExpansionState,
};

pub(super) enum OpenSubmenu<'view, SubmenuKey, Message> {
    Closed,
    Open {
        row_offset_px: f32,
        path: Vec<SubmenuKey>,
        entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
    },
}

pub(super) enum BesideCard<'view, Message> {
    Absent,
    Card {
        row_offset_px: f32,
        card: Element<'view, Message>,
    },
}

pub(super) struct BuiltPanel<'view, SubmenuKey, Message> {
    rows: Vec<Element<'view, Message>>,
    open_submenu: OpenSubmenu<'view, SubmenuKey, Message>,
    beside_card: BesideCard<'view, Message>,
}

impl<'view, SubmenuKey: Clone + PartialEq + 'view, Message: Clone + 'view>
    MenuBuilder<'view, SubmenuKey, Message>
{
    pub(super) fn satellite_reach(
        &self,
        entries: &[MenuEntry<'view, SubmenuKey, Message>],
    ) -> SatelliteReach {
        let submenu_open = entries.iter().any(|entry| {
            matches!(
                entry,
                MenuEntry::Submenu(submenu)
                    if self.submenu_expansion(&submenu.key) == ExpansionState::Expanded
            )
        });
        if submenu_open {
            SatelliteReach::Open(self.chrome.orientation.direction)
        } else {
            SatelliteReach::Closed
        }
    }

    pub(super) fn cascading_panel(
        &self,
        entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
        ancestors: &[SubmenuKey],
    ) -> BuiltPanel<'view, SubmenuKey, Message> {
        let entries = without_redundant_dividers(entries);
        let corners = ListEndCorners::new(entries.len(), self.chrome.style.frame_corners());
        let satellite_reach = self.satellite_reach(&entries);
        let caret_column = match self.chrome.orientation.direction {
            CascadeDirection::Leftward
                if entries
                    .iter()
                    .any(|entry| matches!(entry, MenuEntry::Submenu(_))) =>
            {
                IconColumn::Reserved
            }
            CascadeDirection::Leftward | CascadeDirection::Rightward => IconColumn::Omitted,
        };
        let icon_side = match caret_column {
            IconColumn::Reserved => IconSide::Trailing,
            IconColumn::Omitted => IconSide::Leading,
        };
        let icon_column = shared_icon_column(entries.iter().filter_map(|entry| match entry {
            MenuEntry::Item(item) => Some(&item.row.content),
            MenuEntry::Submenu(submenu) => Some(&submenu.header),
            MenuEntry::Divider => None,
        }));
        let row_offsets: Vec<f32> = entries
            .iter()
            .scan(0.0, |next_offset_px, entry| {
                let row_offset_px = *next_offset_px;
                *next_offset_px += self.entry_height_px(entry);
                Some(row_offset_px)
            })
            .collect();
        entries.into_iter().zip(row_offsets).enumerate().fold(
            BuiltPanel {
                rows: Vec::new(),
                open_submenu: OpenSubmenu::Closed,
                beside_card: BesideCard::Absent,
            },
            |mut panel, (position, (entry, row_offset_px))| {
                let row_corners = corners.radius_at(position);
                let row = match entry {
                    MenuEntry::Divider => self.divider(),
                    MenuEntry::Item(MenuItem {
                        mut row,
                        beside,
                        wrapping,
                    }) => {
                        if let BesideRow::Card(card) = beside {
                            panel.beside_card = BesideCard::Card {
                                row_offset_px,
                                card,
                            };
                        }
                        row.content = row.content.within(icon_column, caret_column, icon_side);
                        wrapping.applied(gestured_menu_row(row, self.chrome.metrics, row_corners))
                    }
                    MenuEntry::Submenu(MenuSubmenu {
                        key,
                        header,
                        entries,
                    }) => {
                        let path: Vec<SubmenuKey> =
                            ancestors.iter().cloned().chain([key.clone()]).collect();
                        let submenu_contents = SubmenuContents::from(entries.as_slice());
                        let expansion_state = match submenu_contents {
                            SubmenuContents::Entries => self.submenu_expansion(&key),
                            SubmenuContents::Nothing => ExpansionState::Collapsed,
                        };
                        let row = gestured_menu_row(
                            self.header_row(
                                header.within(icon_column, caret_column, icon_side),
                                &path,
                                expansion_state,
                                satellite_reach,
                                submenu_contents,
                            ),
                            self.chrome.metrics,
                            row_corners,
                        );
                        if expansion_state == ExpansionState::Expanded {
                            panel.open_submenu = OpenSubmenu::Open {
                                row_offset_px,
                                path,
                                entries,
                            };
                        }
                        row
                    }
                };
                panel.rows.push(row);
                panel
            },
        )
    }

    pub(super) fn submenu_panel(
        &self,
        entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
        path: &[SubmenuKey],
        anchor: NotchRowAnchor,
    ) -> NotchedPanel<'view, Message> {
        let depth = path.len();
        let BuiltPanel {
            rows,
            open_submenu,
            beside_card,
        } = self.cascading_panel(entries, path);
        let scrolled_px = self.state.scrolled_px(depth);
        let list = self.scrolled_list(rows, depth);
        let nested_panels = match open_submenu {
            OpenSubmenu::Closed => Vec::new(),
            OpenSubmenu::Open {
                row_offset_px,
                path,
                entries,
            } => vec![self.submenu_panel(
                entries,
                &path,
                NotchRowAnchor::WithinParent {
                    row_offset: row_offset_px - scrolled_px,
                },
            )],
        };
        let card_panels = match beside_card {
            BesideCard::Absent => Vec::new(),
            BesideCard::Card {
                row_offset_px,
                card,
            } => vec![self.panel(
                SatelliteRole::Beside,
                card,
                NotchRowAnchor::WithinParent {
                    row_offset: row_offset_px - scrolled_px,
                },
            )],
        };
        self.panel(SatelliteRole::ItemList, list, anchor)
            .beside(nested_panels.into_iter().chain(card_panels))
    }

    pub(super) fn cascading_surfaces(
        &self,
        entries: Vec<MenuEntry<'view, SubmenuKey, Message>>,
    ) -> NestedMenuSurfaces<'view, Message> {
        let BuiltPanel {
            rows,
            open_submenu,
            beside_card,
        } = self.cascading_panel(entries, &[]);
        let scrolled_px = self.state.scrolled_px(0);
        let primary = self.scrolled_list(rows, 0);
        let submenu_satellites = match open_submenu {
            OpenSubmenu::Closed => Vec::new(),
            OpenSubmenu::Open {
                row_offset_px,
                path,
                entries,
            } => vec![
                self.submenu_panel(
                    entries,
                    &path,
                    NotchRowAnchor::FromPrimaryTop(row_offset_px - scrolled_px),
                )
                .into(),
            ],
        };
        NestedMenuSurfaces {
            primary,
            satellites: submenu_satellites
                .into_iter()
                .chain(self.primary_beside_card(beside_card, scrolled_px))
                .collect(),
        }
    }

    pub(super) fn primary_beside_card(
        &self,
        beside_card: BesideCard<'view, Message>,
        scrolled_px: f32,
    ) -> Vec<Element<'view, Message>> {
        match beside_card {
            BesideCard::Absent => Vec::new(),
            BesideCard::Card {
                row_offset_px,
                card,
            } => vec![
                self.panel(
                    SatelliteRole::Beside,
                    card,
                    NotchRowAnchor::FromPrimaryTop(row_offset_px - scrolled_px),
                )
                .into(),
            ],
        }
    }
}
