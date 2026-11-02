use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
};

use iced::{
    Alignment, Element, Padding,
    widget::{Column, Row, Space, container},
};
use sottomano_config::Config;

use crate::{
    group_items::aligned_quantity::AlignedQuantity,
    group_menus::menu_rows::menu_row::menu_row_height,
    widget_groups::{widget_group_content::GroupEvent, widget_group_identity::WidgetGroupTitle},
    widgets::text::{column_heading, row_key_label},
};

pub(crate) struct QuantityTableColumn<'a, M> {
    pub group_title: WidgetGroupTitle,
    pub aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
}

pub(crate) fn quantity_table<'a, M: 'a>(
    members: Vec<QuantityTableColumn<'a, M>>,
    config: &'a Config,
) -> Option<Element<'a, GroupEvent<M>>> {
    if members.is_empty() {
        return None;
    }
    let font = &config.theme.font;
    let overlay_surfaces = &config.overlay_surfaces;
    let quantity_rows =
        aligned_quantity_rows(members.iter().map(|member| member.aligned.keys().copied()));
    let row_height = menu_row_height(config);
    let table_cell = |content: Element<'a, GroupEvent<M>>| -> Element<'a, GroupEvent<M>> {
        container(content).center_y(row_height).into()
    };
    let label_column = Column::with_children(
        once(Element::from(Space::new()))
            .chain(
                quantity_rows
                    .iter()
                    .map(|quantity| row_key_label(font, quantity.to_string()).into()),
            )
            .map(table_cell),
    )
    .align_x(Alignment::End);
    let member_columns = members.into_iter().map(|mut member| {
        Column::with_children(
            once(Element::from(column_heading(
                font,
                member.group_title.to_string(),
            )))
            .chain(quantity_rows.iter().map(|quantity| {
                member
                    .aligned
                    .remove(quantity)
                    .unwrap_or_else(|| Space::new().into())
            }))
            .map(table_cell),
        )
        .into()
    });
    Some(
        container(
            Row::with_children(once(label_column.into()).chain(member_columns))
                .spacing(overlay_surfaces.column_gap),
        )
        .padding(Padding::ZERO.horizontal(overlay_surfaces.content_inset))
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

#[cfg(test)]
mod tests {
    use super::aligned_quantity_rows;
    use crate::group_items::aligned_quantity::AlignedQuantity;

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
