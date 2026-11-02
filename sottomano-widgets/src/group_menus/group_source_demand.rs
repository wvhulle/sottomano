use iced::Subscription;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupSurface {
    Hidden,
    WidgetGroupFace,
    AllRows,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupSourceDemand {
    surface: GroupSurface,
}

impl GroupSourceDemand {
    #[must_use]
    pub const fn on_panel() -> Self {
        Self {
            surface: GroupSurface::WidgetGroupFace,
        }
    }

    pub(crate) fn in_menu(menu_open: bool, on_panel: bool, item_pinned: bool) -> Self {
        let surface = if menu_open || item_pinned {
            GroupSurface::AllRows
        } else if on_panel {
            GroupSurface::WidgetGroupFace
        } else {
            GroupSurface::Hidden
        };
        Self { surface }
    }

    #[must_use]
    pub fn expanded_when(mut self, expanded: bool) -> Self {
        if expanded && self.surface == GroupSurface::WidgetGroupFace {
            self.surface = GroupSurface::AllRows;
        }
        self
    }

    fn shows_face_reading(self) -> bool {
        self.surface != GroupSurface::Hidden
    }

    fn shows_row_reading(self) -> bool {
        self.surface == GroupSurface::AllRows
    }

    pub fn while_face_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        subscription_when(self.shows_face_reading(), subscription)
    }

    pub fn while_rows_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        subscription_when(self.shows_row_reading(), subscription)
    }

    pub fn while_any_reading_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        subscription_when(self.shows_face_reading(), subscription)
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

    #[test]
    fn a_hidden_group_runs_no_reading() {
        let demand = GroupSourceDemand::in_menu(false, false, false);
        assert!(!demand.shows_face_reading());
        assert!(!demand.shows_row_reading());
    }

    #[test]
    fn an_open_menu_runs_every_row() {
        assert!(GroupSourceDemand::in_menu(true, false, false).shows_row_reading());
    }

    #[test]
    fn a_collapsed_panel_group_runs_only_its_face() {
        let demand = GroupSourceDemand::on_panel();
        assert!(demand.shows_face_reading());
        assert!(!demand.shows_row_reading());
    }

    #[test]
    fn an_expanded_panel_group_runs_every_row() {
        assert!(
            GroupSourceDemand::on_panel()
                .expanded_when(true)
                .shows_row_reading()
        );
    }

    #[test]
    fn expanding_is_ignored_while_the_group_is_hidden() {
        let demand = GroupSourceDemand::in_menu(false, false, false).expanded_when(true);
        assert!(!demand.shows_row_reading());
    }

    #[test]
    fn a_pinned_item_runs_every_row_of_its_family() {
        let demand = GroupSourceDemand::in_menu(false, false, true);
        assert!(demand.shows_face_reading());
        assert!(demand.shows_row_reading());
    }
}
