use corredo::transitions::ExpansionState;
use iced::Subscription;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupSurface {
    Hidden,
    SummaryOnly,
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
            surface: GroupSurface::SummaryOnly,
        }
    }

    pub(crate) const fn showing(surface: GroupSurface) -> Self {
        Self { surface }
    }

    #[must_use]
    pub fn expanded_when(mut self, expansion_state: ExpansionState) -> Self {
        if expansion_state == ExpansionState::Expanded && self.surface == GroupSurface::SummaryOnly
        {
            self.surface = GroupSurface::AllRows;
        }
        self
    }

    pub fn while_summary_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        match self.surface {
            GroupSurface::SummaryOnly | GroupSurface::AllRows => subscription,
            GroupSurface::Hidden => Subscription::none(),
        }
    }

    pub fn while_rows_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        match self.surface {
            GroupSurface::AllRows => subscription,
            GroupSurface::SummaryOnly | GroupSurface::Hidden => Subscription::none(),
        }
    }

    pub fn while_any_reading_shown<M>(self, subscription: Subscription<M>) -> Subscription<M> {
        self.while_summary_shown(subscription)
    }
}

#[cfg(test)]
mod tests {
    use super::{ExpansionState, GroupSourceDemand, GroupSurface};

    #[test]
    fn an_expanded_panel_group_runs_every_row() {
        assert_eq!(
            GroupSourceDemand::on_panel()
                .expanded_when(ExpansionState::Expanded)
                .surface,
            GroupSurface::AllRows
        );
    }

    #[test]
    fn expanding_is_ignored_while_the_group_is_hidden() {
        let demand = GroupSourceDemand::showing(GroupSurface::Hidden)
            .expanded_when(ExpansionState::Expanded);
        assert_eq!(demand.surface, GroupSurface::Hidden);
    }
}
