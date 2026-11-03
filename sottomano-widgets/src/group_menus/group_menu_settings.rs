use sottomano_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        toggle_control::{IconToggle, SwitchPresses},
    },
    group_menus::{
        group_menu_presentation::{
            GroupMenuLayout, GroupMenuOpenTrigger, GroupMenuPresentation, HoverCardPlacement,
            OpenGroupLimit,
        },
        group_menu_state::{GroupMenuMessage, GroupMenuState},
    },
    hover_cards::{HoverTarget, hover_card_body::hover_card_source_delivery::HoverCard},
    widget_groups::{
        widget_group_content::{WidgetGroupContent, WidgetGroupFace},
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        layer_appearance_controls::AppearanceTarget,
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMenuSettingsTarget {
    AccordionLayout,
    PressToReveal,
    KeepOneGroupOpen,
    InlineCards,
    Appearance(AppearanceTarget),
}

impl From<GroupMenuSettingsTarget> for HoverTarget {
    fn from(target: GroupMenuSettingsTarget) -> Self {
        HoverTarget::GroupMenuSettings(target)
    }
}

fn accordion_layout_toggle(presentation: GroupMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Accordion menus",
        Phosphor::Rows(Weight::Regular),
        presentation.layout == GroupMenuLayout::Accordion,
        SwitchPresses {
            turning_on: "open a group's items below its row",
            turning_off: "open a group's items in a panel beside its row",
        },
    )
}

fn press_to_reveal_toggle(presentation: GroupMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Press to open menus",
        Phosphor::CursorClick(Weight::Regular),
        presentation.open_trigger == GroupMenuOpenTrigger::Press,
        SwitchPresses {
            turning_on: "open menus and cascading groups by pressing them, until pressed away",
            turning_off: "open menus and cascading groups by hovering them, until the pointer \
                          leaves",
        },
    )
}

fn keep_one_group_open_toggle(presentation: GroupMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Keep one group open",
        Phosphor::ListDashes(Weight::Regular),
        presentation.accordion_open_limit == OpenGroupLimit::One,
        SwitchPresses {
            turning_on: "close the open group when another opens",
            turning_off: "keep groups open until pressed again",
        },
    )
}

fn inline_cards_toggle(presentation: GroupMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Inline cards",
        Phosphor::Cards(Weight::Regular),
        presentation.hover_card_placement == HoverCardPlacement::InlineWithRow,
        SwitchPresses {
            turning_on: "open an item's card below its row on press",
            turning_off: "open an item's card beside its row on hover",
        },
    )
}

impl GroupMenuState {
    #[must_use]
    pub fn settings_content<'a>(
        &'a self,
        config: &'a Config,
    ) -> WidgetGroupContent<'a, GroupMenuMessage> {
        let presentation = self.presentation();
        let accordion_only_item = (presentation.layout == GroupMenuLayout::Accordion).then(|| {
            GroupItem::toggle(
                keep_one_group_open_toggle(presentation),
                GroupMenuSettingsTarget::KeepOneGroupOpen,
                GroupMenuMessage::SwitchPresentation(presentation.with_open_limit_toggled()),
            )
        });
        WidgetGroupContent::expandable(
            WidgetGroupFamily::GroupMenuSettings,
            self.settings_expansion(),
            GroupMenuMessage::SettingsExpansion,
            [
                GroupItem::toggle(
                    accordion_layout_toggle(presentation),
                    GroupMenuSettingsTarget::AccordionLayout,
                    GroupMenuMessage::SwitchPresentation(presentation.with_layout_toggled()),
                ),
                GroupItem::toggle(
                    press_to_reveal_toggle(presentation),
                    GroupMenuSettingsTarget::PressToReveal,
                    GroupMenuMessage::SwitchPresentation(presentation.with_open_trigger_toggled()),
                ),
            ]
            .into_iter()
            .chain(accordion_only_item)
            .chain([GroupItem::toggle(
                inline_cards_toggle(presentation),
                GroupMenuSettingsTarget::InlineCards,
                GroupMenuMessage::SwitchPresentation(presentation.with_card_placement_toggled()),
            )])
            .chain(
                self.menu_appearance_controls()
                    .items(config, GroupMenuMessage::Appearance),
            ),
        )
        .face(WidgetGroupFace::Icon {
            glyph: Phosphor::ListDashes(Weight::Regular).into(),
        })
    }

    #[must_use]
    pub fn settings_hover_card(&self, target: GroupMenuSettingsTarget, clock: Clock) -> HoverCard {
        let presentation = self.presentation();
        match target {
            GroupMenuSettingsTarget::AccordionLayout => {
                accordion_layout_toggle(presentation).switch_hover_card(clock)
            }
            GroupMenuSettingsTarget::PressToReveal => {
                press_to_reveal_toggle(presentation).switch_hover_card(clock)
            }
            GroupMenuSettingsTarget::KeepOneGroupOpen => {
                keep_one_group_open_toggle(presentation).switch_hover_card(clock)
            }
            GroupMenuSettingsTarget::InlineCards => {
                inline_cards_toggle(presentation).switch_hover_card(clock)
            }
            GroupMenuSettingsTarget::Appearance(target) => self
                .menu_appearance_controls()
                .hover_card_for(target, clock),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupMenuMessage, GroupMenuPresentation, GroupMenuState};
    use crate::group_menus::group_menu_presentation::{GroupMenuLayout, OpenGroupLimit};

    #[test]
    fn switching_presentation_keeps_only_the_first_open_group_at_a_limit_of_one() {
        use crate::{
            group_menus::group_menu_state::GroupMenuTopic,
            widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        };
        let several_open = GroupMenuPresentation {
            layout: GroupMenuLayout::Accordion,
            ..GroupMenuPresentation::default()
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(several_open));
        for family in [WidgetGroupFamily::Gpu, WidgetGroupFamily::Compute] {
            let _ = group_menus.update(GroupMenuMessage::ToggleMenuGroup {
                menu: GroupMenuTopic::Hardware,
                group_identity: WidgetGroupIdentity::sole(family),
            });
        }
        assert_eq!(group_menus.open_groups(GroupMenuTopic::Hardware).len(), 2);
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(
            GroupMenuPresentation {
                accordion_open_limit: OpenGroupLimit::One,
                ..several_open
            },
        ));
        assert_eq!(group_menus.open_groups(GroupMenuTopic::Hardware).len(), 1);
    }

    #[test]
    fn a_loaded_presentation_reads_back_unchanged() {
        let stored = GroupMenuPresentation {
            layout: GroupMenuLayout::Accordion,
            ..GroupMenuPresentation::default()
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(stored));
        assert_eq!(group_menus.presentation(), stored);
    }
}
