use crate::{hover_tooltips::InspectedRow, widget_groups::group_menus::GroupMenuKind};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum MenuInspection {
    #[default]
    Closed,
    Member(Box<OpenMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct OpenMember {
    menu: GroupMenuKind,
    member: usize,
    row: RowInspection,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum RowInspection {
    #[default]
    Nothing,
    Row(InspectedRow),
}

impl MenuInspection {
    #[must_use]
    pub(super) fn inspecting_member(self, menu: GroupMenuKind, member: usize) -> Self {
        let row = match self {
            MenuInspection::Member(open) if open.menu == menu && open.member == member => open.row,
            MenuInspection::Member(_) | MenuInspection::Closed => RowInspection::Nothing,
        };
        MenuInspection::Member(Box::new(OpenMember { menu, member, row }))
    }

    #[must_use]
    pub(super) fn inspecting_row(self, menu: GroupMenuKind, inspected: InspectedRow) -> Self {
        match self {
            MenuInspection::Member(open) if open.menu == menu => {
                MenuInspection::Member(Box::new(OpenMember {
                    row: RowInspection::Row(inspected),
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    pub(super) fn open_member(&self, menu: GroupMenuKind) -> Option<usize> {
        match self {
            MenuInspection::Member(open) if open.menu == menu => Some(open.member),
            MenuInspection::Member(_) | MenuInspection::Closed => None,
        }
    }

    pub(super) fn inspected_row(&self, menu: GroupMenuKind) -> Option<&InspectedRow> {
        match self {
            MenuInspection::Member(open) if open.menu == menu => match &open.row {
                RowInspection::Row(row) => Some(row),
                RowInspection::Nothing => None,
            },
            MenuInspection::Member(_) | MenuInspection::Closed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MenuInspection;
    use crate::{
        hover_tooltips::{HoverAnnouncement, HoverTarget, InspectedRow},
        widget_groups::group_menus::GroupMenuKind,
    };

    fn inspected_workspace_row() -> InspectedRow {
        InspectedRow {
            target: HoverTarget::Workspace(7),
            announcement: HoverAnnouncement::default(),
        }
    }

    #[test]
    fn reentering_the_open_member_keeps_its_inspected_row() {
        let inspection = MenuInspection::Closed
            .inspecting_member(GroupMenuKind::Hardware, 2)
            .inspecting_row(GroupMenuKind::Hardware, inspected_workspace_row())
            .inspecting_member(GroupMenuKind::Hardware, 2);
        assert_eq!(
            inspection.inspected_row(GroupMenuKind::Hardware),
            Some(&inspected_workspace_row())
        );
    }

    #[test]
    fn opening_another_member_clears_the_inspected_row() {
        let inspection = MenuInspection::Closed
            .inspecting_member(GroupMenuKind::Hardware, 2)
            .inspecting_row(GroupMenuKind::Hardware, inspected_workspace_row())
            .inspecting_member(GroupMenuKind::Hardware, 3);
        assert_eq!(inspection.open_member(GroupMenuKind::Hardware), Some(3));
        assert_eq!(inspection.inspected_row(GroupMenuKind::Hardware), None);
    }

    #[test]
    fn another_menu_sees_no_open_member() {
        let inspection = MenuInspection::Closed.inspecting_member(GroupMenuKind::Hardware, 1);
        assert_eq!(inspection.open_member(GroupMenuKind::Behaviour), None);
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let inspection = MenuInspection::Closed
            .inspecting_row(GroupMenuKind::Hardware, inspected_workspace_row());
        assert_eq!(inspection, MenuInspection::Closed);
    }
}
