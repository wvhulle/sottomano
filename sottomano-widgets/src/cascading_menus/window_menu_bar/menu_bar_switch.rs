use sottomano_desktop_sources::desktop_windows_apps::x11_window_menu_properties::X11MenuLookupError;

use super::{WindowMenuBar, WindowMenuBarInteraction};
use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    hover_cards::hover_card_body::{Section, SectionWidget},
    indicator_groups::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::IndicatorGroupFamily,
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
    },
    widgets::phosphor::{Phosphor, Weight},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMenuBarUse {
    Enabled,
    Disabled,
}

impl WindowMenuBarUse {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            WindowMenuBarUse::Enabled => WindowMenuBarUse::Disabled,
            WindowMenuBarUse::Disabled => WindowMenuBarUse::Enabled,
        }
    }
}

impl SingletonFlagState for WindowMenuBarUse {
    const STORED_FLAG: SingletonFlag = SingletonFlag::WindowMenuBarUse;
    const STORED_AS_SET: Self = WindowMenuBarUse::Enabled;
    const STORED_AS_CLEARED: Self = WindowMenuBarUse::Disabled;
}

impl From<WindowMenuBarUse> for SwitchPosition {
    fn from(window_menu_bar_use: WindowMenuBarUse) -> Self {
        match window_menu_bar_use {
            WindowMenuBarUse::Enabled => SwitchPosition::On,
            WindowMenuBarUse::Disabled => SwitchPosition::Off,
        }
    }
}

fn window_menu_bar_use_toggle(window_menu_bar_use: WindowMenuBarUse) -> IconToggle {
    IconToggle::new(
        "Window menu bar",
        Phosphor::AppWindow(Weight::Regular),
        window_menu_bar_use,
        SwitchPresses {
            turning_on: "show the focused window's menus in a panel menu; Qt apps started \
                         afterwards hide their own menu bar",
            turning_off: "leave menus inside their windows",
        },
    )
}

struct MenuLookupFailureIndicator<'failure> {
    lookup_failure: &'failure X11MenuLookupError,
}

impl<'failure> MenuLookupFailureIndicator<'failure> {
    fn indicator(self) -> GroupIndicator<'failure> {
        GroupIndicator::glyph_only(
            "Menu lookup",
            Phosphor::Warning(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("failed".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Error",
            SectionWidget::paragraph(self.lookup_failure.to_string()),
        )]
    }
}

impl WindowMenuBar {
    pub fn switch_content(&self) -> IndicatorGroupContent<'_> {
        let mut rows = vec![GroupIndicator::toggle(
            window_menu_bar_use_toggle(self.window_menu_bar_use),
            WindowMenuBarInteraction::ToggleUse,
        )];
        if let Some(lookup_failure) = self.lookup_failure() {
            rows.push(MenuLookupFailureIndicator { lookup_failure }.indicator());
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::WindowMenuBarSwitch,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::WindowMenuBarSwitch.title(),
                Phosphor::CursorClick(Weight::Regular).into(),
                match self.lookup_failure() {
                    Some(_) => IndicatorState::Failed,
                    None => IndicatorState::Idle,
                },
            ),
            &self.switch_expansion,
            WindowMenuBarInteraction::SwitchExpansion,
            rows,
        )
    }
}
