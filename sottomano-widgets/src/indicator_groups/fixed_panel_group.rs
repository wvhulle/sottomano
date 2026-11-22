use corredo::popovers::NestedMenuOpenTrigger;
use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    group_indicators::group_indicator::{GroupIndicator, RowHoverCard},
    hover_cards::{CardHeader, HoverTarget, MenuHandleIndicator},
    indicator_groups::indicator_group_content::{GroupEvent, group_panel_view::fixed_view},
};

pub enum FixedPanelGroup<'view> {
    Empty,
    Shown {
        indicator: Box<GroupIndicator<'view>>,
        hover_target: HoverTarget,
    },
}

impl<'view> FixedPanelGroup<'view> {
    #[must_use]
    pub fn handle_target(&self) -> Option<&HoverTarget> {
        match self {
            FixedPanelGroup::Empty => None,
            FixedPanelGroup::Shown { hover_target, .. } => Some(hover_target),
        }
    }

    #[must_use]
    pub fn hover_card(&self, target: &HoverTarget, clock: Clock) -> Option<RowHoverCard> {
        match self {
            FixedPanelGroup::Shown {
                indicator,
                hover_target,
            } if hover_target == target => Some(RowHoverCard {
                sections: indicator.card_with_actions(clock),
                header: CardHeader::Unheaded,
            }),
            FixedPanelGroup::Shown { .. } | FixedPanelGroup::Empty => None,
        }
    }

    #[must_use]
    pub fn view(
        self,
        config: &'view Config,
        panel_position: Position,
        open_trigger: NestedMenuOpenTrigger,
        handle_indicator: MenuHandleIndicator,
        clock: Clock,
    ) -> Option<Element<'view, GroupEvent>> {
        let FixedPanelGroup::Shown {
            indicator,
            hover_target,
        } = self
        else {
            return None;
        };
        let indicator = indicator.opening_card_on(open_trigger);
        Some(fixed_view(
            &indicator,
            hover_target,
            Severity::default(),
            handle_indicator,
            config,
            panel_position,
            clock,
        ))
    }
}
