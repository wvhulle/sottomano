use iced::Subscription;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupSurface {
    Hidden,
    HandleFace,
    AllRows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSourceDemand<'a> {
    surface: GroupSurface,
    pinned_item_names: Vec<&'a str>,
}

impl<'a> GroupSourceDemand<'a> {
    #[must_use]
    pub const fn on_bar() -> Self {
        Self {
            surface: GroupSurface::HandleFace,
            pinned_item_names: Vec::new(),
        }
    }

    pub(super) fn in_menu(
        menu_open: bool,
        on_bar: bool,
        pinned_item_names: impl IntoIterator<Item = &'a str>,
    ) -> Self {
        let surface = if menu_open {
            GroupSurface::AllRows
        } else if on_bar {
            GroupSurface::HandleFace
        } else {
            GroupSurface::Hidden
        };
        Self {
            surface,
            pinned_item_names: pinned_item_names.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn expanded_when(mut self, expanded: bool) -> Self {
        if expanded && self.surface == GroupSurface::HandleFace {
            self.surface = GroupSurface::AllRows;
        }
        self
    }

    fn pins_any_item(&self, item_names: &[&str]) -> bool {
        item_names
            .iter()
            .any(|item_name| self.pinned_item_names.contains(item_name))
    }

    fn shows_face_reading(&self, item_names: &[&str]) -> bool {
        self.surface != GroupSurface::Hidden || self.pins_any_item(item_names)
    }

    fn shows_row_reading(&self, item_names: &[&str]) -> bool {
        self.surface == GroupSurface::AllRows || self.pins_any_item(item_names)
    }

    fn shows_any_reading(&self) -> bool {
        self.surface != GroupSurface::Hidden || !self.pinned_item_names.is_empty()
    }

    pub fn while_face_shown<M>(
        &self,
        item_names: &[&str],
        subscription: Subscription<M>,
    ) -> Subscription<M> {
        subscription_when(self.shows_face_reading(item_names), subscription)
    }

    pub fn while_rows_shown<M>(
        &self,
        item_names: &[&str],
        subscription: Subscription<M>,
    ) -> Subscription<M> {
        subscription_when(self.shows_row_reading(item_names), subscription)
    }

    pub fn while_any_reading_shown<M>(&self, subscription: Subscription<M>) -> Subscription<M> {
        subscription_when(self.shows_any_reading(), subscription)
    }
}

fn subscription_when<M>(wanted: bool, subscription: Subscription<M>) -> Subscription<M> {
    if wanted {
        subscription
    } else {
        Subscription::none()
    }
}

#[cfg(test)]
mod tests {
    use super::GroupSourceDemand;

    const WIND_ITEM: &[&str] = &["Wind"];
    const NO_ITEMS: [&str; 0] = [];

    fn hidden_group() -> GroupSourceDemand<'static> {
        GroupSourceDemand::in_menu(false, false, NO_ITEMS)
    }

    fn bar_group() -> GroupSourceDemand<'static> {
        GroupSourceDemand::in_menu(false, true, NO_ITEMS)
    }

    fn open_menu_group() -> GroupSourceDemand<'static> {
        GroupSourceDemand::in_menu(true, false, NO_ITEMS)
    }

    #[test]
    fn hidden_group_shows_no_reading() {
        let demand = hidden_group();
        assert!(!demand.shows_face_reading(WIND_ITEM));
        assert!(!demand.shows_row_reading(WIND_ITEM));
        assert!(!demand.shows_any_reading());
    }

    #[test]
    fn open_menu_shows_every_row_whether_pinned_or_not() {
        for demand in [
            open_menu_group(),
            GroupSourceDemand::in_menu(true, true, NO_ITEMS),
        ] {
            assert!(demand.shows_face_reading(WIND_ITEM));
            assert!(demand.shows_row_reading(WIND_ITEM));
        }
    }

    #[test]
    fn collapsed_bar_group_shows_only_its_face() {
        let demand = bar_group();
        assert!(demand.shows_face_reading(WIND_ITEM));
        assert!(!demand.shows_row_reading(WIND_ITEM));
    }

    #[test]
    fn expanded_bar_group_shows_every_row() {
        assert!(bar_group().expanded_when(true).shows_row_reading(WIND_ITEM));
    }

    #[test]
    fn groups_placed_on_the_bar_outside_a_menu_follow_the_same_rule() {
        assert_eq!(GroupSourceDemand::on_bar(), bar_group());
        assert_eq!(
            GroupSourceDemand::on_bar().expanded_when(true),
            bar_group().expanded_when(true)
        );
    }

    #[test]
    fn expanded_flag_is_ignored_while_the_group_is_hidden() {
        let demand = hidden_group().expanded_when(true);
        assert_eq!(demand, hidden_group());
        assert!(!demand.shows_row_reading(WIND_ITEM));
    }

    #[test]
    fn pinned_item_shows_only_that_item_of_a_hidden_group() {
        let demand = GroupSourceDemand::in_menu(false, false, ["Wind"]);
        assert!(demand.shows_row_reading(WIND_ITEM));
        assert!(demand.shows_face_reading(WIND_ITEM));
        assert!(!demand.shows_row_reading(&["Pollen"]));
        assert!(demand.shows_any_reading());
    }
}
