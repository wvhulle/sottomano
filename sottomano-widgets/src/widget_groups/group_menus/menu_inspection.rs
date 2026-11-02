use crate::{hover_tooltips::InspectedRow, widget_groups::group_menus::GroupMenuTopic};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum GroupMenuInspection {
    #[default]
    Closed,
    Member(Box<OpenMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct OpenMember {
    menu: GroupMenuTopic,
    member: usize,
    row: RowInspection,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum RowInspection {
    #[default]
    Nothing,
    Row(InspectedRow),
}

impl GroupMenuInspection {
    #[must_use]
    pub(super) fn inspecting_member(self, menu: GroupMenuTopic, member: usize) -> Self {
        let row = match self {
            GroupMenuInspection::Member(open) if open.menu == menu && open.member == member => {
                open.row
            }
            GroupMenuInspection::Member(_) | GroupMenuInspection::Closed => RowInspection::Nothing,
        };
        GroupMenuInspection::Member(Box::new(OpenMember { menu, member, row }))
    }

    #[must_use]
    pub(super) fn inspecting_row(self, menu: GroupMenuTopic, inspected: InspectedRow) -> Self {
        match self {
            GroupMenuInspection::Member(open) if open.menu == menu => {
                GroupMenuInspection::Member(Box::new(OpenMember {
                    row: RowInspection::Row(inspected),
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    pub(super) fn open_member(&self, menu: GroupMenuTopic) -> Option<usize> {
        match self {
            GroupMenuInspection::Member(open) if open.menu == menu => Some(open.member),
            GroupMenuInspection::Member(_) | GroupMenuInspection::Closed => None,
        }
    }

    pub(super) fn inspected_row(&self, menu: GroupMenuTopic) -> Option<&InspectedRow> {
        match self {
            GroupMenuInspection::Member(open) if open.menu == menu => match &open.row {
                RowInspection::Row(row) => Some(row),
                RowInspection::Nothing => None,
            },
            GroupMenuInspection::Member(_) | GroupMenuInspection::Closed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupMenuInspection;
    use crate::{
        hover_tooltips::{HoverAnnouncement, HoverTarget, InspectedRow},
        widget_groups::group_menus::GroupMenuTopic,
    };

    fn inspected_workspace_row() -> InspectedRow {
        InspectedRow {
            target: HoverTarget::Workspace(7),
            announcement: HoverAnnouncement::default(),
        }
    }

    #[test]
    fn reentering_the_open_member_keeps_its_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .inspecting_member(GroupMenuTopic::Hardware, 2)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .inspecting_member(GroupMenuTopic::Hardware, 2);
        assert_eq!(
            inspection.inspected_row(GroupMenuTopic::Hardware),
            Some(&inspected_workspace_row())
        );
    }

    #[test]
    fn opening_another_member_clears_the_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .inspecting_member(GroupMenuTopic::Hardware, 2)
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row())
            .inspecting_member(GroupMenuTopic::Hardware, 3);
        assert_eq!(inspection.open_member(GroupMenuTopic::Hardware), Some(3));
        assert_eq!(inspection.inspected_row(GroupMenuTopic::Hardware), None);
    }

    #[test]
    fn another_menu_sees_no_open_member() {
        let inspection = GroupMenuInspection::Closed.inspecting_member(GroupMenuTopic::Hardware, 1);
        assert_eq!(inspection.open_member(GroupMenuTopic::Behaviour), None);
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let inspection = GroupMenuInspection::Closed
            .inspecting_row(GroupMenuTopic::Hardware, inspected_workspace_row());
        assert_eq!(inspection, GroupMenuInspection::Closed);
    }
}
