use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
    vec,
};

use chrono::Locale;
use iced::{
    Alignment, Element, Padding,
    alignment::Horizontal,
    widget::{Row, Space, container},
};
use sottomano_config::{Config, screen_edges::Position};

use crate::{
    clock::Clock,
    hover_tooltips::{
        GroupHandle, HoverAnnouncement, HoverTarget, PressHints, announcing_hoverable,
    },
    widgets::{
        aligned_quantity::AlignedQuantity,
        bar_widget::Glyph,
        configured_theme::text_color,
        content_sized_table::{SpannableRow, content_sized_spanning_grid},
        keyed_record_table::separator_rule,
        pager_slot::PagerSlot,
        text::{column_heading, row_key_label},
        widget_group::{DrawerCells, EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests},
    },
};

pub struct GroupDrawer<'a, M> {
    drawer: GroupHandle,
    group_title: &'static str,
    glyph: Glyph,
    members: Vec<GroupContent<'a, M>>,
}

impl<'a, M: Clone + 'a> GroupDrawer<'a, M> {
    pub fn new(
        drawer: GroupHandle,
        group_title: &'static str,
        glyph: impl Into<Glyph>,
        members: impl IntoIterator<Item = GroupContent<'a, M>>,
    ) -> Self {
        Self {
            drawer,
            group_title,
            glyph: glyph.into(),
            members: members.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn drawer(&self) -> GroupHandle {
        self.drawer
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        self.members
            .iter()
            .map(|member| member.toast_requests(locale, clock))
            .collect()
    }

    pub fn handle_view<Outer: From<GroupEvent<M>> + 'a>(
        &self,
        config: &'a Config,
        bar_position: Position,
        clock: Clock,
    ) -> Element<'a, Outer> {
        let highest_alert_severity = self
            .members
            .iter()
            .map(|member| member.highest_alert_severity(clock))
            .max()
            .unwrap_or_default();
        let handle = announcing_hoverable(
            self.glyph.view(config.theme.font.icon_size_px, text_color),
            HoverTarget::GroupHandle(self.drawer),
            HoverAnnouncement::from_group_handle(self.group_title, PressHints::default()),
            GroupEvent::Tooltip,
        );
        let indicator = EdgeIndicator {
            highest_alert_severity,
            cursor_hovering: false,
            expanded: false,
        }
        .view(config, 0.0);
        PagerSlot::new(handle, indicator)
            .view(config, bar_position)
            .map(Outer::from)
    }

    #[must_use]
    pub fn sheet<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        clock: Clock,
    ) -> Option<Element<'a, Outer>> {
        sheet_view(self.members, config, clock).map(|sheet| sheet.map(Outer::from))
    }
}

struct MemberRemainder<'a, M> {
    aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
    unaligned: vec::IntoIter<Element<'a, GroupEvent<M>>>,
}

fn sheet_view<'a, M: Clone + 'a>(
    members: Vec<GroupContent<'a, M>>,
    config: &'a Config,
    clock: Clock,
) -> Option<Element<'a, GroupEvent<M>>> {
    let member_cells: Vec<DrawerCells<'a, M>> = members
        .into_iter()
        .filter_map(|member| member.drawer_cells(config, clock))
        .collect();
    if member_cells.is_empty() {
        return None;
    }
    let quantity_rows = aligned_quantity_rows(
        member_cells
            .iter()
            .map(|cells| cells.aligned.keys().copied()),
    );
    let unaligned_depth = member_cells
        .iter()
        .map(|cells| cells.unaligned.len())
        .max()
        .unwrap_or_default();
    let column_count = member_cells.len() + 1;
    let (faces, mut remainders): (Vec<_>, Vec<_>) = member_cells
        .into_iter()
        .map(|cells| {
            (
                drawer_column_header(config, cells.face, cells.group_title),
                MemberRemainder {
                    aligned: cells.aligned,
                    unaligned: cells.unaligned.into_iter(),
                },
            )
        })
        .unzip();
    let header_row: Vec<Element<'a, GroupEvent<M>>> =
        once(Space::new().into()).chain(faces).collect();
    let aligned_rows: Vec<Vec<Element<'a, GroupEvent<M>>>> = quantity_rows
        .iter()
        .map(|quantity| {
            once(row_key_label(&config.theme.font, quantity.to_string()).into())
                .chain(remainders.iter_mut().map(|remainder| {
                    remainder
                        .aligned
                        .remove(quantity)
                        .unwrap_or_else(|| Space::new().into())
                }))
                .collect()
        })
        .collect();
    let unaligned_rows: Vec<Vec<Element<'a, GroupEvent<M>>>> = (0..unaligned_depth)
        .map(|_| {
            once(Space::new().into())
                .chain(remainders.iter_mut().map(|remainder| {
                    remainder
                        .unaligned
                        .next()
                        .unwrap_or_else(|| Space::new().into())
                }))
                .collect()
        })
        .collect();
    let section_separator = (!aligned_rows.is_empty() && !unaligned_rows.is_empty())
        .then(|| SpannableRow::FullWidth(separator_rule()));
    let grid = content_sized_spanning_grid(
        once(Horizontal::Right)
            .chain(vec![Horizontal::Left; column_count - 1])
            .collect(),
        once(header_row)
            .chain(aligned_rows)
            .map(SpannableRow::Columned)
            .chain(section_separator)
            .chain(unaligned_rows.into_iter().map(SpannableRow::Columned)),
    )
    .padding_x(config.layout.overlay.row_spacing)
    .padding_y(config.layout.overlay.row_spacing / 2.0);
    Some(
        container(grid)
            .padding(Padding::ZERO.horizontal(config.layout.overlay.content_inset))
            .into(),
    )
}

fn aligned_quantity_rows(
    member_quantities: impl IntoIterator<Item = impl IntoIterator<Item = AlignedQuantity>>,
) -> Vec<AlignedQuantity> {
    member_quantities
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn drawer_column_header<'a, M: 'a>(
    config: &Config,
    face: Element<'a, M>,
    group_title: &'static str,
) -> Element<'a, M> {
    Row::with_children([face, column_heading(&config.theme.font, group_title).into()])
        .spacing(config.layout.panel.widget_spacing)
        .align_y(Alignment::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::aligned_quantity_rows;
    use crate::widgets::aligned_quantity::AlignedQuantity;

    #[test]
    fn rows_are_the_union_of_member_quantities_in_declared_order() {
        let panels = [AlignedQuantity::Blur, AlignedQuantity::Opacity];
        let niri = [AlignedQuantity::CornerRadius];
        assert_eq!(
            aligned_quantity_rows([panels.to_vec(), niri.to_vec()]),
            [
                AlignedQuantity::Opacity,
                AlignedQuantity::CornerRadius,
                AlignedQuantity::Blur,
            ]
        );
    }

    #[test]
    fn a_quantity_shared_by_several_members_is_one_row() {
        let shared = [AlignedQuantity::CornerRadius];
        assert_eq!(
            aligned_quantity_rows([shared.to_vec(), shared.to_vec(), shared.to_vec()]),
            [AlignedQuantity::CornerRadius]
        );
    }

    #[test]
    fn members_without_aligned_items_add_no_rows() {
        assert!(aligned_quantity_rows(Vec::<Vec<AlignedQuantity>>::new()).is_empty());
    }
}
