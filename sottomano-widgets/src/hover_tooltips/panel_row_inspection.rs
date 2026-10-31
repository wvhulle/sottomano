use crate::hover_tooltips::{HoverTarget, InspectedRow};

#[derive(Debug, Clone, Default, PartialEq)]
pub enum PanelRowInspection {
    #[default]
    Nothing,
    Member(Box<MemberInspection>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberInspection {
    within: HoverTarget,
    member: usize,
    row: MemberRowInspection,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum MemberRowInspection {
    #[default]
    Nothing,
    Row(InspectedRow),
}

impl PanelRowInspection {
    #[must_use]
    pub fn inspecting_member(self, within: &HoverTarget, member: usize) -> Self {
        let row = match self {
            PanelRowInspection::Member(current)
                if &current.within == within && current.member == member =>
            {
                current.row
            }
            PanelRowInspection::Member(_) | PanelRowInspection::Nothing => {
                MemberRowInspection::Nothing
            }
        };
        PanelRowInspection::Member(Box::new(MemberInspection {
            within: within.clone(),
            member,
            row,
        }))
    }

    #[must_use]
    pub fn inspecting_row(self, within: &HoverTarget, inspected: InspectedRow) -> Self {
        match self {
            PanelRowInspection::Member(current) if &current.within == within => {
                PanelRowInspection::Member(Box::new(MemberInspection {
                    row: MemberRowInspection::Row(inspected),
                    ..*current
                }))
            }
            unrelated => unrelated,
        }
    }

    #[must_use]
    pub fn member_within(&self, open_target: Option<&HoverTarget>) -> Option<usize> {
        match self {
            PanelRowInspection::Member(current) if open_target == Some(&current.within) => {
                Some(current.member)
            }
            PanelRowInspection::Member(_) | PanelRowInspection::Nothing => None,
        }
    }

    #[must_use]
    pub fn row_within(&self, open_target: Option<&HoverTarget>) -> Option<&InspectedRow> {
        match self {
            PanelRowInspection::Member(current) if open_target == Some(&current.within) => {
                match &current.row {
                    MemberRowInspection::Row(row) => Some(row),
                    MemberRowInspection::Nothing => None,
                }
            }
            PanelRowInspection::Member(_) | PanelRowInspection::Nothing => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PanelRowInspection;
    use crate::hover_tooltips::{GroupHandle, HoverAnnouncement, HoverTarget, InspectedRow};

    fn hardware_menu() -> HoverTarget {
        HoverTarget::GroupHandle(GroupHandle::HardwareMenu)
    }

    fn inspected_workspace_row() -> InspectedRow {
        InspectedRow {
            target: HoverTarget::Workspace(7),
            announcement: HoverAnnouncement::default(),
        }
    }

    #[test]
    fn reentering_the_open_member_keeps_its_inspected_row() {
        let group_menu = hardware_menu();
        let inspection = PanelRowInspection::Nothing
            .inspecting_member(&group_menu, 2)
            .inspecting_row(&group_menu, inspected_workspace_row())
            .inspecting_member(&group_menu, 2);
        assert_eq!(
            inspection.row_within(Some(&group_menu)),
            Some(&inspected_workspace_row())
        );
    }

    #[test]
    fn opening_another_member_clears_the_inspected_row() {
        let group_menu = hardware_menu();
        let inspection = PanelRowInspection::Nothing
            .inspecting_member(&group_menu, 2)
            .inspecting_row(&group_menu, inspected_workspace_row())
            .inspecting_member(&group_menu, 3);
        assert_eq!(inspection.member_within(Some(&group_menu)), Some(3));
        assert_eq!(inspection.row_within(Some(&group_menu)), None);
    }

    #[test]
    fn inspection_is_forgotten_once_another_target_opens() {
        let group_menu = hardware_menu();
        let inspection = PanelRowInspection::Nothing.inspecting_member(&group_menu, 1);
        let other_group_menu = HoverTarget::GroupHandle(GroupHandle::BehaviourMenu);
        assert_eq!(inspection.member_within(Some(&other_group_menu)), None);
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let group_menu = hardware_menu();
        let inspection =
            PanelRowInspection::Nothing.inspecting_row(&group_menu, inspected_workspace_row());
        assert_eq!(inspection, PanelRowInspection::Nothing);
    }
}
