use corredo::popovers::{NestedMenuLayout, NestedMenuPresentation};
use sottomano_toml_config::Config;

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPresses},
    },
    group_menus::group_menu_state::{GroupMenuMessage, GroupMenuState},
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::phosphor::{Phosphor, Weight},
};

fn accordion_layout_toggle(presentation: NestedMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Accordion menus",
        Phosphor::Rows(Weight::Regular),
        presentation.layout,
        SwitchPresses {
            turning_on: "open a group's items below its row",
            turning_off: "open a group's items in a panel beside its row",
        },
    )
}

fn click_to_open_menus_toggle(presentation: NestedMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Click to open menus",
        Phosphor::CursorClick(Weight::Regular),
        presentation.open_trigger,
        SwitchPresses {
            turning_on: "open a menu by clicking its panel icon, until clicked away",
            turning_off: "open a menu by hovering its panel icon, until the pointer leaves",
        },
    )
}

fn keep_one_group_open_toggle(presentation: NestedMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Keep one group open",
        Phosphor::ListDashes(Weight::Regular),
        presentation.accordion_open_limit,
        SwitchPresses {
            turning_on: "close the open group when another opens",
            turning_off: "keep groups open until pressed again",
        },
    )
}

fn submenu_connector_toggle(presentation: NestedMenuPresentation) -> IconToggle {
    IconToggle::new(
        "Submenu connector",
        Phosphor::ChatTeardrop(Weight::Regular),
        presentation.submenu_connector,
        SwitchPresses {
            turning_on: "join a panel beside a row to that row with a pointed connector",
            turning_off: "set a panel beside a row apart, without a connector",
        },
    )
}

impl GroupMenuState {
    #[must_use]
    pub fn settings_content<'a>(&'a self, config: &'a Config) -> IndicatorGroupContent<'a> {
        let presentation = self.presentation;
        let accordion_only_indicator =
            (presentation.layout == NestedMenuLayout::Accordion).then(|| {
                GroupIndicator::toggle(
                    keep_one_group_open_toggle(presentation),
                    GroupMenuMessage::SwitchPresentation(presentation.with_open_limit_toggled()),
                )
            });
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::GroupMenuSettings,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::GroupMenuSettings.title(),
                Phosphor::ListDashes(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.settings_expansion,
            GroupMenuMessage::SettingsExpansion,
            [
                GroupIndicator::toggle(
                    accordion_layout_toggle(presentation),
                    GroupMenuMessage::SwitchPresentation(presentation.with_layout_toggled()),
                ),
                GroupIndicator::toggle(
                    click_to_open_menus_toggle(presentation),
                    GroupMenuMessage::SwitchPresentation(presentation.with_open_trigger_toggled()),
                ),
                GroupIndicator::toggle(
                    submenu_connector_toggle(presentation),
                    GroupMenuMessage::SwitchPresentation(
                        presentation.with_submenu_connector_toggled(),
                    ),
                ),
            ]
            .into_iter()
            .chain(accordion_only_indicator)
            .chain(self.layer.indicators(config, GroupMenuMessage::Appearance)),
        )
    }
}

#[cfg(test)]
mod tests {
    use corredo::popovers::{NestedMenuLayout, OpenGroupLimit};

    use super::{GroupMenuMessage, GroupMenuState, NestedMenuPresentation};

    #[test]
    fn switching_presentation_keeps_only_the_first_open_group_at_a_limit_of_one() {
        use crate::{
            group_menus::group_menu_state::GroupMenuTopic,
            indicator_groups::indicator_group_identity::{
                IndicatorGroupFamily, IndicatorGroupIdentity,
            },
        };
        let several_open = NestedMenuPresentation {
            layout: NestedMenuLayout::Accordion,
            ..NestedMenuPresentation::default()
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(several_open));
        for family in [IndicatorGroupFamily::Gpu, IndicatorGroupFamily::Compute] {
            let _ = group_menus.update(GroupMenuMessage::ToggleMenuGroup {
                menu: GroupMenuTopic::Hardware,
                group_identity: IndicatorGroupIdentity::sole(family),
            });
        }
        assert_eq!(group_menus.open_groups(GroupMenuTopic::Hardware).len(), 2);
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(
            NestedMenuPresentation {
                accordion_open_limit: OpenGroupLimit::One,
                ..several_open
            },
        ));
        assert_eq!(group_menus.open_groups(GroupMenuTopic::Hardware).len(), 1);
    }

    #[test]
    fn a_loaded_presentation_reads_back_unchanged() {
        let stored = NestedMenuPresentation {
            layout: NestedMenuLayout::Accordion,
            ..NestedMenuPresentation::default()
        };
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(stored));
        assert_eq!(group_menus.presentation, stored);
    }
}
