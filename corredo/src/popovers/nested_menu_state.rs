#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NestedMenuLayout {
    #[default]
    Cascading,
    Accordion,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NestedMenuOpenTrigger {
    Hover,
    #[default]
    Press,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OpenGroupLimit {
    One,
    #[default]
    Several,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SubmenuConnector {
    #[default]
    Hidden,
    Shown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NestedMenuPresentation {
    pub layout: NestedMenuLayout,
    pub open_trigger: NestedMenuOpenTrigger,
    pub accordion_open_limit: OpenGroupLimit,
    pub submenu_connector: SubmenuConnector,
}

impl NestedMenuPresentation {
    #[must_use]
    pub const fn open_group_limit(self) -> OpenGroupLimit {
        match self.layout {
            NestedMenuLayout::Cascading => OpenGroupLimit::One,
            NestedMenuLayout::Accordion => self.accordion_open_limit,
        }
    }

    #[must_use]
    pub const fn with_layout_toggled(self) -> Self {
        Self {
            layout: match self.layout {
                NestedMenuLayout::Cascading => NestedMenuLayout::Accordion,
                NestedMenuLayout::Accordion => NestedMenuLayout::Cascading,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_trigger_toggled(self) -> Self {
        Self {
            open_trigger: match self.open_trigger {
                NestedMenuOpenTrigger::Hover => NestedMenuOpenTrigger::Press,
                NestedMenuOpenTrigger::Press => NestedMenuOpenTrigger::Hover,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_submenu_connector_toggled(self) -> Self {
        Self {
            submenu_connector: match self.submenu_connector {
                SubmenuConnector::Hidden => SubmenuConnector::Shown,
                SubmenuConnector::Shown => SubmenuConnector::Hidden,
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_open_limit_toggled(self) -> Self {
        Self {
            accordion_open_limit: match self.accordion_open_limit {
                OpenGroupLimit::One => OpenGroupLimit::Several,
                OpenGroupLimit::Several => OpenGroupLimit::One,
            },
            ..self
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum NestedMenuInspection<MenuKey, GroupKey, RowKey> {
    #[default]
    Closed,
    Open(Box<OpenNestedMenuRows<MenuKey, GroupKey, RowKey>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenNestedMenuRows<MenuKey, GroupKey, RowKey> {
    menu: MenuKey,
    open_groups: Vec<GroupKey>,
    row: RowInspection<RowKey>,
    scrolled_px: f32,
    item_list_scrolled_px: f32,
}

#[derive(Debug, Clone, PartialEq)]
enum RowInspection<RowKey> {
    Nothing,
    Row(RowKey),
}

impl<MenuKey: PartialEq, GroupKey: PartialEq, RowKey>
    NestedMenuInspection<MenuKey, GroupKey, RowKey>
{
    #[must_use]
    pub fn revealing_group(self, menu: MenuKey, group_identity: GroupKey) -> Self {
        let (row, scrolled_px, item_list_scrolled_px) = match self {
            NestedMenuInspection::Open(open)
                if open.menu == menu && open.open_groups.contains(&group_identity) =>
            {
                (open.row, open.scrolled_px, open.item_list_scrolled_px)
            }
            NestedMenuInspection::Open(open) if open.menu == menu => {
                (RowInspection::Nothing, open.scrolled_px, 0.0)
            }
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => {
                (RowInspection::Nothing, 0.0, 0.0)
            }
        };
        NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
            menu,
            open_groups: vec![group_identity],
            row,
            scrolled_px,
            item_list_scrolled_px,
        }))
    }

    #[must_use]
    pub fn toggling_group(
        self,
        menu: MenuKey,
        group_identity: GroupKey,
        open_limit: OpenGroupLimit,
    ) -> Self {
        let (mut open_groups, row, scrolled_px) = match self {
            NestedMenuInspection::Open(open) if open.menu == menu => {
                (open.open_groups, open.row, open.scrolled_px)
            }
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => {
                (Vec::new(), RowInspection::Nothing, 0.0)
            }
        };
        if open_groups.contains(&group_identity) {
            open_groups.retain(|open_group| *open_group != group_identity);
            return NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
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
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
                    menu,
                    open_groups,
                    row: RowInspection::Nothing,
                    scrolled_px,
                    item_list_scrolled_px: 0.0,
                }))
            }
            OpenGroupLimit::Several => {
                open_groups.push(group_identity);
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
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
    pub fn inspecting_row(self, menu: &MenuKey, inspected: RowKey) -> Self {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => {
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
                    row: RowInspection::Row(inspected),
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    #[must_use]
    pub fn adopting_open_limit(self, open_limit: OpenGroupLimit) -> Self {
        match (self, open_limit) {
            (NestedMenuInspection::Open(mut open), OpenGroupLimit::One) => {
                open.open_groups.truncate(1);
                NestedMenuInspection::Open(open)
            }
            (unchanged, _) => unchanged,
        }
    }

    #[must_use]
    pub fn scrolled_to(self, menu: MenuKey, scrolled_px: f32) -> Self {
        match self {
            NestedMenuInspection::Open(open) if open.menu == menu => {
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
                    row: RowInspection::Nothing,
                    scrolled_px,
                    ..*open
                }))
            }
            NestedMenuInspection::Closed => {
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
                    menu,
                    open_groups: Vec::new(),
                    row: RowInspection::Nothing,
                    scrolled_px,
                    item_list_scrolled_px: 0.0,
                }))
            }
            unrelated @ NestedMenuInspection::Open(_) => unrelated,
        }
    }

    #[must_use]
    pub fn item_list_scrolled_to(self, menu: &MenuKey, scrolled_px: f32) -> Self {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => {
                NestedMenuInspection::Open(Box::new(OpenNestedMenuRows {
                    row: RowInspection::Nothing,
                    item_list_scrolled_px: scrolled_px,
                    ..*open
                }))
            }
            unrelated => unrelated,
        }
    }

    #[must_use]
    pub fn item_list_scrolled_px(&self, menu: &MenuKey) -> f32 {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => open.item_list_scrolled_px,
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => 0.0,
        }
    }

    #[must_use]
    pub fn scrolled_px(&self, menu: &MenuKey) -> f32 {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => open.scrolled_px,
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => 0.0,
        }
    }

    #[must_use]
    pub fn open_groups(&self, menu: &MenuKey) -> &[GroupKey] {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => &open.open_groups,
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => &[],
        }
    }

    #[must_use]
    pub fn inspected_target(&self, menu: &MenuKey) -> Option<&RowKey> {
        match self {
            NestedMenuInspection::Open(open) if open.menu == *menu => match &open.row {
                RowInspection::Row(row) => Some(row),
                RowInspection::Nothing => None,
            },
            NestedMenuInspection::Open(_) | NestedMenuInspection::Closed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NestedMenuInspection, NestedMenuOpenTrigger, NestedMenuPresentation, OpenGroupLimit,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum MenuTopic {
        Hardware,
        Behaviour,
    }

    const GPU_GROUP: &str = "gpu";
    const COMPUTE_GROUP: &str = "compute";

    type GroupMenuInspection = NestedMenuInspection<MenuTopic, &'static str, u32>;

    fn inspected_item_row() -> u32 {
        7
    }

    #[test]
    fn reentering_the_open_member_keeps_its_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .revealing_group(MenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(&MenuTopic::Hardware, inspected_item_row())
            .revealing_group(MenuTopic::Hardware, GPU_GROUP);
        assert_eq!(
            inspection.inspected_target(&MenuTopic::Hardware),
            Some(&inspected_item_row())
        );
    }

    #[test]
    fn opening_another_member_clears_the_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .revealing_group(MenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(&MenuTopic::Hardware, inspected_item_row())
            .revealing_group(MenuTopic::Hardware, COMPUTE_GROUP);
        assert_eq!(
            inspection.open_groups(&MenuTopic::Hardware),
            [COMPUTE_GROUP]
        );
        assert_eq!(inspection.inspected_target(&MenuTopic::Hardware), None);
    }

    #[test]
    fn another_menu_sees_no_open_member() {
        let inspection =
            GroupMenuInspection::Closed.revealing_group(MenuTopic::Hardware, GPU_GROUP);
        assert!(inspection.open_groups(&MenuTopic::Behaviour).is_empty());
    }

    #[test]
    fn a_row_without_an_open_member_is_ignored() {
        let inspection =
            GroupMenuInspection::Closed.inspecting_row(&MenuTopic::Hardware, inspected_item_row());
        assert_eq!(inspection, GroupMenuInspection::Closed);
    }

    #[test]
    fn one_open_group_closes_the_previous_group() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(MenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::One)
            .toggling_group(MenuTopic::Hardware, COMPUTE_GROUP, OpenGroupLimit::One);
        assert_eq!(
            inspection.open_groups(&MenuTopic::Hardware),
            [COMPUTE_GROUP]
        );
    }

    #[test]
    fn several_open_groups_stay_open() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(MenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .toggling_group(MenuTopic::Hardware, COMPUTE_GROUP, OpenGroupLimit::Several);
        assert_eq!(
            inspection.open_groups(&MenuTopic::Hardware),
            [GPU_GROUP, COMPUTE_GROUP]
        );
    }

    #[test]
    fn collapsing_a_group_clears_its_inspected_row() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(MenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .inspecting_row(&MenuTopic::Hardware, inspected_item_row())
            .toggling_group(MenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several);
        assert!(inspection.open_groups(&MenuTopic::Hardware).is_empty());
        assert_eq!(inspection.inspected_target(&MenuTopic::Hardware), None);
    }

    #[test]
    fn adopting_a_limit_of_one_keeps_only_the_first_open_group() {
        let inspection = GroupMenuInspection::Closed
            .toggling_group(MenuTopic::Hardware, GPU_GROUP, OpenGroupLimit::Several)
            .toggling_group(MenuTopic::Hardware, COMPUTE_GROUP, OpenGroupLimit::Several)
            .adopting_open_limit(OpenGroupLimit::One);
        assert_eq!(inspection.open_groups(&MenuTopic::Hardware), [GPU_GROUP]);
    }

    #[test]
    fn opening_another_group_resets_the_item_list_scroll() {
        let scrolled = GroupMenuInspection::Closed
            .revealing_group(MenuTopic::Hardware, GPU_GROUP)
            .inspecting_row(&MenuTopic::Hardware, inspected_item_row())
            .item_list_scrolled_to(&MenuTopic::Hardware, 40.0);
        assert_eq!(scrolled.inspected_target(&MenuTopic::Hardware), None);
        assert!((scrolled.item_list_scrolled_px(&MenuTopic::Hardware) - 40.0).abs() < f32::EPSILON);

        let same_group = scrolled
            .clone()
            .revealing_group(MenuTopic::Hardware, GPU_GROUP);
        assert!(
            (same_group.item_list_scrolled_px(&MenuTopic::Hardware) - 40.0).abs() < f32::EPSILON
        );

        let other_group = scrolled.revealing_group(MenuTopic::Hardware, COMPUTE_GROUP);
        assert!(
            other_group
                .item_list_scrolled_px(&MenuTopic::Hardware)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn switching_layouts_keeps_each_sub_choice() {
        let chosen = NestedMenuPresentation::default()
            .with_open_trigger_toggled()
            .with_open_limit_toggled();
        let round_trip = chosen.with_layout_toggled().with_layout_toggled();
        assert_eq!(round_trip, chosen);
        assert_eq!(chosen.open_trigger, NestedMenuOpenTrigger::Hover);
        assert_eq!(chosen.accordion_open_limit, OpenGroupLimit::One);
    }

    #[test]
    fn a_cascading_menu_opens_one_group_whatever_the_accordion_limit() {
        let several_stored = NestedMenuPresentation {
            accordion_open_limit: OpenGroupLimit::Several,
            ..NestedMenuPresentation::default()
        };
        assert_eq!(several_stored.open_group_limit(), OpenGroupLimit::One);
    }
}
