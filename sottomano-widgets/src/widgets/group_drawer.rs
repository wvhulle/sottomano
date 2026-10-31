use std::{collections::BTreeSet, iter::once};

use chrono::Locale;
use iced::{
    Alignment, Element,
    alignment::Horizontal,
    widget::{Row, Space, container},
};
use sottomano_config::{Config, screen_edges::Position};

use crate::{
    clock::Clock,
    hover_tooltips::{
        GroupHandle, HoverAnnouncement, HoverTarget, PopupMessage, PressHints,
        announcing_hoverable, tooltip_content::TooltipContent,
    },
    widgets::{
        aligned_quantity::AlignedQuantity,
        bar_widget::Glyph,
        configured_theme::text_color,
        content_sized_table::{SpannableRow, content_sized_spanning_grid},
        keyed_record_table::separator_rule,
        pager_slot::PagerSlot,
        text::{column_heading, row_key_label},
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{
            DrawerCells, EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests,
            NamedDrawerCell,
        },
        window_icon::WindowAssets,
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

    pub fn inspecting_sheet<Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a>(
        self,
        config: &'a Config,
        assets: WindowAssets<'a>,
        clock: Clock,
        inspected: &Tooltip,
    ) -> Option<Element<'a, Outer>> {
        let sheet = self.sheet(config, clock)?;
        if inspected.is_empty() {
            return Some(sheet);
        }
        let detail_pane =
            Element::from(TooltipContent::new(inspected, config, assets, clock).view())
                .map(Outer::from);
        Some(
            Row::with_children([
                sheet,
                container(detail_pane)
                    .width(config.layout.overlay.drawer_detail_width)
                    .into(),
            ])
            .spacing(config.layout.overlay.column_spacing)
            .into(),
        )
    }
}

type GridCells<'a, M> = Vec<Element<'a, GroupEvent<M>>>;
type GridRow<'a, M> = SpannableRow<'a, GridCells<'a, M>, GroupEvent<M>>;

struct PivotAndSectionRows<'a, M> {
    pivot_rows: Vec<GridCells<'a, M>>,
    section_rows: Vec<GridRow<'a, M>>,
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
    let pivot_column_count = member_cells
        .iter()
        .filter(|cells| !cells.aligned.is_empty())
        .count();
    let column_count = pivot_column_count.max(1) + 1;
    let pads_row = move |cells: GridCells<'a, M>| -> GridCells<'a, M> {
        let padding = column_count.saturating_sub(cells.len());
        cells
            .into_iter()
            .chain((0..padding).map(|_| Space::new().into()))
            .collect()
    };
    let PivotAndSectionRows {
        pivot_rows,
        section_rows,
    } = pivot_and_section_rows(member_cells, &quantity_rows, config, pivot_column_count);
    let section_separator = (!pivot_rows.is_empty() && !section_rows.is_empty())
        .then(|| SpannableRow::FullWidth(separator_rule()));
    let grid = content_sized_spanning_grid(
        once(Horizontal::Right)
            .chain(vec![Horizontal::Left; column_count - 1])
            .collect(),
        pivot_rows
            .into_iter()
            .map(pads_row)
            .map(SpannableRow::Columned)
            .chain(section_separator)
            .chain(section_rows.into_iter().map(|row| match row {
                SpannableRow::Columned(cells) => SpannableRow::Columned(pads_row(cells)),
                full_width @ SpannableRow::FullWidth(_) => full_width,
            })),
    )
    .padding_x(config.layout.overlay.row_spacing)
    .padding_y(config.layout.overlay.row_spacing / 2.0);
    Some(
        container(grid)
            .padding(config.layout.overlay.content_inset)
            .into(),
    )
}

fn pivot_and_section_rows<'a, M: Clone + 'a>(
    member_cells: Vec<DrawerCells<'a, M>>,
    quantity_rows: &[AlignedQuantity],
    config: &'a Config,
    pivot_column_count: usize,
) -> PivotAndSectionRows<'a, M> {
    let font = &config.theme.font;
    let mut pivot_faces = Vec::with_capacity(pivot_column_count);
    let mut pivot_columns = Vec::with_capacity(pivot_column_count);
    let mut section_rows: Vec<GridRow<'a, M>> = Vec::new();
    for cells in member_cells {
        let DrawerCells {
            column_face,
            section_face,
            group_title,
            aligned,
            unaligned,
        } = cells;
        if !aligned.is_empty() {
            pivot_faces.push(drawer_column_header(config, column_face, group_title));
            pivot_columns.push(aligned);
        }
        if unaligned.is_empty() {
            continue;
        }
        section_rows.push(SpannableRow::FullWidth(drawer_column_header(
            config,
            section_face,
            group_title,
        )));
        section_rows.extend(unaligned.into_iter().map(|cell: NamedDrawerCell<'a, M>| {
            SpannableRow::Columned(vec![row_key_label(font, cell.name).into(), cell.view])
        }));
    }
    if pivot_columns.is_empty() {
        return PivotAndSectionRows {
            pivot_rows: Vec::new(),
            section_rows,
        };
    }
    let header_row = once(Space::new().into()).chain(pivot_faces).collect();
    let quantity_cells = quantity_rows.iter().map(|quantity| {
        once(row_key_label(font, quantity.to_string()).into())
            .chain(pivot_columns.iter_mut().map(|column| {
                column
                    .remove(quantity)
                    .unwrap_or_else(|| Space::new().into())
            }))
            .collect()
    });
    PivotAndSectionRows {
        pivot_rows: once(header_row).chain(quantity_cells).collect(),
        section_rows,
    }
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
