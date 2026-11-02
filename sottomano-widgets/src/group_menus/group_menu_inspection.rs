use crate::{
    group_menus::group_menu_state::GroupMenuTopic, hover_cards::HoverTarget,
    widget_groups::widget_group_identity::WidgetGroupIdentity,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) enum GroupMenuInspection {
    #[default]
    Closed,
    Group(Box<OpenMenuGroup>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenMenuGroup {
    menu: GroupMenuTopic,
    group_identity: WidgetGroupIdentity,
    row: RowInspection,
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
        let row = match self {
            GroupMenuInspection::Group(open)
                if open.menu == menu && open.group_identity == group_identity =>
            {
                open.row
            }
            GroupMenuInspection::Group(_) | GroupMenuInspection::Closed => RowInspection::Nothing,
        };
        GroupMenuInspection::Group(Box::new(OpenMenuGroup {
            menu,
            group_identity,
            row,
        }))
    }

    #[must_use]
    pub(crate) fn inspecting_row(self, menu: GroupMenuTopic, inspected: HoverTarget) -> Self {
        match self {
            GroupMenuInspection::Group(open) if open.menu == menu => {
                GroupMenuInspection::Group(Box::new(OpenMenuGroup {
                    row: RowInspection::Row(inspected),
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    pub(crate) fn open_group(&self, menu: GroupMenuTopic) -> Option<&WidgetGroupIdentity> {
        match self {
            GroupMenuInspection::Group(open) if open.menu == menu => Some(&open.group_identity),
            GroupMenuInspection::Group(_) | GroupMenuInspection::Closed => None,
        }
    }

    pub(crate) fn inspected_target(&self, menu: GroupMenuTopic) -> Option<&HoverTarget> {
        match self {
            GroupMenuInspection::Group(open) if open.menu == menu => match &open.row {
                RowInspection::Row(row) => Some(row),
                RowInspection::Nothing => None,
            },
            GroupMenuInspection::Group(_) | GroupMenuInspection::Closed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupMenuInspection;
    use crate::{
        group_menus::group_menu_state::GroupMenuTopic,
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
            inspection.open_group(GroupMenuTopic::Hardware),
            Some(&COMPUTE_GROUP)
        );
        assert_eq!(inspection.inspected_target(GroupMenuTopic::Hardware), None);
    }

    #[test]
    fn another_menu_sees_no_open_member() {
        let inspection =
            GroupMenuInspection::Closed.revealing_group(GroupMenuTopic::Hardware, GPU_GROUP);
        assert_eq!(inspection.open_group(GroupMenuTopic::Behaviour), None);
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let inspection = GroupMenuInspection::Closed
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row());
        assert_eq!(inspection, GroupMenuInspection::Closed);
    }
}
