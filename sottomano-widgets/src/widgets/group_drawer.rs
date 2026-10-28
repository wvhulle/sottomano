use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
    vec,
};

use chrono::Locale;
use iced::{
    Alignment, Element, Length,
    alignment::Horizontal,
    widget::{Row, Space, container, text},
};
use sottomano_config::{Config, screen_edges::Position};

use crate::{
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget, PressHints, hoverable},
    widgets::{
        activity_line::activity_line,
        aligned_quantity::AlignedQuantity,
        bar_widget::Glyph,
        configured_theme::{background_strong_color, text_color},
        content_sized_table::content_sized_grid,
        pager_slot::PagerSlot,
        text::bar_text,
        widget_group::{DrawerCells, EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests},
    },
};

pub struct GroupDrawer<'a, M> {
    drawer: GroupHandle,
    glyph: Glyph,
    members: Vec<GroupContent<'a, M>>,
}

impl<'a, M: Clone + 'a> GroupDrawer<'a, M> {
    pub fn new(
        drawer: GroupHandle,
        glyph: impl Into<Glyph>,
        members: impl IntoIterator<Item = GroupContent<'a, M>>,
    ) -> Self {
        Self {
            drawer,
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
        let handle = hoverable(
            self.glyph.view(config.theme.font.icon_size_px, text_color),
            HoverTarget::GroupHandle(self.drawer),
            PressHints::default(),
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
    let member_count = member_cells.len();
    let (faces, mut remainders): (Vec<_>, Vec<_>) = member_cells
        .into_iter()
        .map(|cells| {
            (
                container(cells.face).center_x(Length::Fill).into(),
                MemberRemainder {
                    aligned: cells.aligned,
                    unaligned: cells.unaligned.into_iter(),
                },
            )
        })
        .unzip();
    let rule_row = || {
        (0..=member_count)
            .map(|_| drawer_rule(config))
            .collect::<Vec<_>>()
    };
    let header_row: Vec<Element<'a, GroupEvent<M>>> =
        once(Space::new().into()).chain(faces).collect();
    let aligned_rows: Vec<Vec<Element<'a, GroupEvent<M>>>> = quantity_rows
        .iter()
        .map(|quantity| {
            once(quantity_label(config, *quantity))
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
    let section_rule = (!aligned_rows.is_empty() && !unaligned_rows.is_empty()).then(rule_row);
    let at_least_bar_height =
        |row: Vec<Element<'a, GroupEvent<M>>>| -> Vec<Element<'a, GroupEvent<M>>> {
            row.into_iter()
                .map(|cell| at_least_height(config.layout.panel.height, cell))
                .collect()
        };
    let grid = content_sized_grid(
        vec![Horizontal::Left; member_count + 1],
        once(at_least_bar_height(header_row))
            .chain(once(rule_row()))
            .chain(aligned_rows.into_iter().map(at_least_bar_height))
            .chain(section_rule)
            .chain(unaligned_rows.into_iter().map(at_least_bar_height)),
    )
    .padding_x(config.layout.panel.widget_spacing)
    .padding_y(config.layout.overlay.row_spacing / 2.0);
    Some(
        container(grid)
            .padding(config.layout.panel.group_padding)
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

fn at_least_height<'a, M: 'a>(minimum_height: f32, cell: Element<'a, M>) -> Element<'a, M> {
    Row::with_children([Space::new().height(minimum_height).into(), cell])
        .align_y(Alignment::Center)
        .into()
}

fn quantity_label<'a, M: 'a>(config: &Config, quantity: AlignedQuantity) -> Element<'a, M> {
    bar_text(&config.theme.font, quantity.to_string())
        .style(text::secondary)
        .into()
}

fn drawer_rule<'a, M: 'a>(config: &Config) -> Element<'a, M> {
    activity_line(
        background_strong_color,
        1.0,
        config.panel_bar.indicator_thickness,
    )
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
