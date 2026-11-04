use crate::{
    group_menus::{group_menu_presentation::OpenGroupLimit, group_menu_state::GroupMenuTopic},
    hover_cards::HoverTarget,
    widget_groups::widget_group_identity::WidgetGroupIdentity,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum GroupMenuInspection {
    #[default]
    Closed,
    Open(Box<OpenGroupMenuRows>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenGroupMenuRows {
    menu: GroupMenuTopic,
    open_groups: Vec<WidgetGroupIdentity>,
    row: RowInspection,
    scrolled_px: f32,
    item_list_scrolled_px: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum RowInspection {
    #[default]
    Nothing,
    Row(HoverTarget),
}

impl GroupMenuInspection {
    #[must_use]
    pub(crate) fn revealing_group(
        self,
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
    ) -> Self {
        let (row, scrolled_px, item_list_scrolled_px) = match self {
            GroupMenuInspection::Open(open)
                if open.menu == menu && open.open_groups.contains(&group_identity) =>
            {
                (open.row, open.scrolled_px, open.item_list_scrolled_px)
            }
            GroupMenuInspection::Open(open) if open.menu == menu => {
                (RowInspection::Nothing, open.scrolled_px, 0.0)
            }
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => {
                (RowInspection::Nothing, 0.0, 0.0)
            }
        };
        GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
            menu,
            open_groups: vec![group_identity],
            row,
            scrolled_px,
            item_list_scrolled_px,
        }))
    }

    #[must_use]
    pub(crate) fn toggling_group(
        self,
        menu: GroupMenuTopic,
        group_identity: WidgetGroupIdentity,
        open_limit: OpenGroupLimit,
    ) -> Self {
        let (mut open_groups, row, scrolled_px) = match self {
            GroupMenuInspection::Open(open) if open.menu == menu => {
                (open.open_groups, open.row, open.scrolled_px)
            }
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => {
                (Vec::new(), RowInspection::Nothing, 0.0)
            }
        };
        if open_groups.contains(&group_identity) {
            open_groups.retain(|open_group| *open_group != group_identity);
            return GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                menu,
                open_groups,
                row: RowInspection::Nothing,
                scrolled_px,
                item_list_scrolled_px: 0.0,
            }));
        }
        match open_limit {
            OpenGroupLimit::One => {
                open_groups = vec![group_identity];
                GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                    menu,
                    open_groups,
                    row: RowInspection::Nothing,
                    scrolled_px,
                    item_list_scrolled_px: 0.0,
                }))
            }
            OpenGroupLimit::Several => {
                open_groups.push(group_identity);
                GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                    menu,
                    open_groups,
                    row,
                    scrolled_px,
                    item_list_scrolled_px: 0.0,
                }))
            }
        }
    }

    #[must_use]
    pub(crate) fn inspecting_row(self, menu: GroupMenuTopic, inspected: HoverTarget) -> Self {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => {
                GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                    row: RowInspection::Row(inspected),
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    #[must_use]
    pub(crate) fn adopting_open_limit(self, open_limit: OpenGroupLimit) -> Self {
        match (self, open_limit) {
            (GroupMenuInspection::Open(mut open), OpenGroupLimit::One) => {
                open.open_groups.truncate(1);
                GroupMenuInspection::Open(open)
            }
            (unchanged, _) => unchanged,
        }
    }

    #[must_use]
    pub(crate) fn scrolled_to(self, menu: GroupMenuTopic, scrolled_px: f32) -> Self {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => {
                GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                    row: RowInspection::Nothing,
                    scrolled_px,
                    ..*open
                }))
            }
            GroupMenuInspection::Closed => GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                menu,
                open_groups: Vec::new(),
                row: RowInspection::Nothing,
                scrolled_px,
                item_list_scrolled_px: 0.0,
            })),
            unrelated @ GroupMenuInspection::Open(_) => unrelated,
        }
    }

    #[must_use]
    pub(crate) fn item_list_scrolled_to(self, menu: GroupMenuTopic, scrolled_px: f32) -> Self {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => {
                GroupMenuInspection::Open(Box::new(OpenGroupMenuRows {
                    row: RowInspection::Nothing,
                    item_list_scrolled_px: scrolled_px,
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    pub(crate) fn item_list_scrolled_px(&self, menu: GroupMenuTopic) -> f32 {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => open.item_list_scrolled_px,
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => 0.0,
        }
    }

    pub(crate) fn scrolled_px(&self, menu: GroupMenuTopic) -> f32 {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => open.scrolled_px,
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => 0.0,
        }
    }

    pub(crate) fn open_groups(&self, menu: GroupMenuTopic) -> &[WidgetGroupIdentity] {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => &open.open_groups,
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => &[],
        }
    }

    pub(crate) fn inspected_target(&self, menu: GroupMenuTopic) -> Option<&HoverTarget> {
        match self {
            GroupMenuInspection::Open(open) if open.menu == menu => match &open.row {
                RowInspection::Row(row) => Some(row),
                RowInspection::Nothing => None,
            },
            GroupMenuInspection::Open(_) | GroupMenuInspection::Closed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupMenuInspection;
    use crate::{
        group_menus::{group_menu_presentation::OpenGroupLimit, group_menu_state::GroupMenuTopic},
        hover_cards::HoverTarget,
        widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
    };

    const GPU_GROUP: WidgetGroupIdentity = WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu);
    const COMPUTE_GROUP: WidgetGroupIdentity =
        WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);

    fn inspected_workspace_row() -> HoverTarget {
        HoverTarget::Workspace(7)
    }

    #[test]
    fn reentering_the_open_member_keeps_its_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .revealing_group(GroupMenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .revealing_group(GroupMenuTopic::Hardware, GPU_GROUP);
        assert_eq!(
            inspection.inspected_target(GroupMenuTopic::Hardware),
            Some(&inspected_workspace_row())
        );
    }

    #[test]
    fn opening_another_member_clears_the_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .revealing_group(GroupMenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .revealing_group(GroupMenuTopic::Hardware, COMPUTE_GROUP);
        assert_eq!(
            inspection.open_groups(GroupMenuTopic::Hardware),
            [COMPUTE_GROUP]
        );
        assert_eq!(inspection.inspected_target(GroupMenuTopic::Hardware), None);
    }

    #[test]
    fn another_menu_sees_no_open_member() {
        let inspection =
            GroupMenuInspection::Closed.revealing_group(GroupMenuTopic::Hardware, GPU_GROUP);
        assert!(inspection.open_groups(GroupMenuTopic::Behaviour).is_empty());
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let inspection = GroupMenuInspection::Closed
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row());
        assert_eq!(inspection, GroupMenuInspection::Closed);
    }

    #[test]
    fn one_open_group_closes_the_previous_group() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(GroupMenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::One)
            .toggling_group(GroupMenuTopic::Hardware, COMPUTE_GROUP, OpenGroupLimit::One);
        assert_eq!(
            inspection.open_groups(GroupMenuTopic::Hardware),
            [COMPUTE_GROUP]
        );
    }

    #[test]
    fn several_open_groups_stay_open() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(GroupMenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .toggling_group(
                GroupMenuTopic::Hardware,
                COMPUTE_GROUP,
                OpenGroupLimit::Several,
            );
        assert_eq!(
            inspection.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP, COMPUTE_GROUP]
        );
    }

    #[test]
    fn collapsing_a_group_clears_its_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(GroupMenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .toggling_group(GroupMenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several);
        assert!(inspection.open_groups(GroupMenuTopic::Hardware).is_empty());
        assert_eq!(inspection.inspected_target(GroupMenuTopic::Hardware), None);
    }

    #[test]
    fn adopting_a_limit_of_one_keeps_only_the_first_open_group() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(GroupMenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .toggling_group(
                GroupMenuTopic::Hardware,
                COMPUTE_GROUP,
                OpenGroupLimit::Several,
            )
            .adopting_open_limit(OpenGroupLimit::One);
        assert_eq!(
            inspection.open_groups(GroupMenuTopic::Hardware),
            [GPU_GROUP]
        );
    }

    #[test]
    fn opening_another_group_resets_the_item_list_scroll() {
        let scrolled = GroupMenuInspection::Closed
            .revealing_group(GroupMenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .item_list_scrolled_to(GroupMenuTopic::Hardware, 40.0);
        assert_eq!(scrolled.inspected_target(GroupMenuTopic::Hardware), None);
        assert!(
            (scrolled.item_list_scrolled_px(GroupMenuTopic::Hardware) - 40.0).abs() < f32::EPSILON
        );

        let same_group = scrolled
            .clone()
            .revealing_group(GroupMenuTopic::Hardware, GPU_GROUP);
        assert!(
            (same_group.item_list_scrolled_px(GroupMenuTopic::Hardware) - 40.0).abs()
                < f32::EPSILON
        );

        let other_group = scrolled.revealing_group(GroupMenuTopic::Hardware, COMPUTE_GROUP);
        assert!(
            other_group
                .item_list_scrolled_px(GroupMenuTopic::Hardware)
                .abs()
                < f32::EPSILON
        );
    }
}
