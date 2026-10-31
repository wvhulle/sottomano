use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
};

use iced::{Element, alignment::Horizontal, widget::Space};
use sottomano_config::Config;

use crate::widgets::{
    aligned_quantity::AlignedQuantity,
    content_sized_table::{SpannableRow, content_sized_spanning_grid},
    text::{column_heading, row_key_label},
    widget_group::GroupEvent,
};

pub(super) struct QuantityTableColumn<'a, M> {
    pub group_title: &'static str,
    pub aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
}

pub(super) fn quantity_table<'a, M: 'a>(
    members: Vec<QuantityTableColumn<'a, M>>,
    config: &'a Config,
) -> Option<Element<'a, GroupEvent<M>>> {
    if members.is_empty() {
        return None;
    }
    let font = &config.theme.font;
    let quantity_rows =
        aligned_quantity_rows(members.iter().map(|member| member.aligned.keys().copied()));
    let column_count = members.len() + 1;
    let header_row = once(Space::new().into())
        .chain(
            members
                .iter()
                .map(|member| column_heading(font, member.group_title).into()),
        )
        .collect();
    let mut columns: Vec<_> = members.into_iter().map(|member| member.aligned).collect();
    let quantity_cells = quantity_rows.iter().map(|quantity| {
        once(row_key_label(font, quantity.to_string()).into())
            .chain(columns.iter_mut().map(|column| {
                column
                    .remove(quantity)
                    .unwrap_or_else(|| Space::new().into())
            }))
            .collect()
    });
    let rows: Vec<Vec<Element<'a, GroupEvent<M>>>> =
        once(header_row).chain(quantity_cells).collect();
    Some(
        content_sized_spanning_grid(
            once(Horizontal::Right)
                .chain(vec![Horizontal::Left; column_count - 1])
                .collect(),
            rows.into_iter().map(SpannableRow::Columned),
        )
        .padding_x(config.layout.overlay.row_spacing)
        .padding_y(config.layout.overlay.row_spacing / 2.0)
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
