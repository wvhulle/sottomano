use corredo::popovers::NestedMenuOpenTrigger;
use iced::Element;
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    group_items::group_item::{GroupItem, RowHoverCard},
    hover_cards::{CardHeader, HoverTarget, MenuHandleIndicator},
    widget_groups::widget_group_content::{GroupEvent, fixed_view},
};

pub enum FixedPanelGroup<'a, M> {
    Empty,
    Shown {
        item: Box<GroupItem<'a, M>>,
        hover_target: HoverTarget,
    },
}

impl<'a, M: Clone + 'a> FixedPanelGroup<'a, M> {
    #[must_use]
    pub fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> FixedPanelGroup<'a, Outer> {
        match self {
            FixedPanelGroup::Empty => FixedPanelGroup::Empty,
            FixedPanelGroup::Shown { item, hover_target } => FixedPanelGroup::Shown {
                item: Box::new((*item).map(lift)),
                hover_target,
            },
        }
    }

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
            FixedPanelGroup::Shown { item, hover_target } if hover_target == target => {
                Some(RowHoverCard {
                    sections: item.card_with_actions(clock),
                    header: CardHeader::Unheaded,
                })
            }
            FixedPanelGroup::Shown { .. } | FixedPanelGroup::Empty => None,
        }
    }

    #[must_use]
    pub fn view(
        self,
        config: &'a Config,
        panel_position: Position,
        open_trigger: NestedMenuOpenTrigger,
        handle_indicator: MenuHandleIndicator,
        clock: Clock,
    ) -> Option<Element<'a, M>>
    where
        M: From<GroupEvent<M>>,
    {
        let FixedPanelGroup::Shown { item, hover_target } = self else {
            return None;
        };
        let item = item.opening_card_on(open_trigger);
        Some(
            fixed_view(
                &item,
                hover_target,
                Severity::default(),
                handle_indicator,
                config,
                panel_position,
                clock,
            )
            .map(M::from),
        )
    }
}
