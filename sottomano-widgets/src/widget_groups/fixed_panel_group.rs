use std::slice;

use iced::Element;
use sottomano_config::{screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    group_items::group_item::GroupItem,
    group_menus::group_menu_presentation::GroupMenuOpenTrigger,
    widget_groups::widget_group_content::{GroupEvent, fixed_view},
};

pub struct FixedPanelGroup<'a, M> {
    item: Option<GroupItem<'a, M>>,
}

impl<'a, M: Clone + 'a> FixedPanelGroup<'a, M> {
    #[must_use]
    pub const fn new(item: Option<GroupItem<'a, M>>) -> Self {
        Self { item }
    }

    #[must_use]
    pub fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> FixedPanelGroup<'a, Outer> {
        FixedPanelGroup {
            item: self.item.map(|item| item.map(lift)),
        }
    }

    #[must_use]
    pub fn view(
        self,
        config: &'a sottomano_config::Config,
        panel_position: Position,
        open_trigger: GroupMenuOpenTrigger,
        clock: Clock,
    ) -> Option<Element<'a, M>>
    where
        M: From<GroupEvent<M>>,
    {
        self.item.map(|item| {
            let item = item.opening_card_on(open_trigger);
            fixed_view(
                slice::from_ref(&item),
                Severity::default(),
                config,
                panel_position,
                clock,
            )
            .map(M::from)
        })
    }
}
