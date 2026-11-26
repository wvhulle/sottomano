use super::{AppLauncherMenu, AppLauncherMessage};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAppListing {
    Hidden,
    Listed,
}

impl OpenAppListing {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            OpenAppListing::Hidden => OpenAppListing::Listed,
            OpenAppListing::Listed => OpenAppListing::Hidden,
        }
    }
}

impl SingletonFlagState for OpenAppListing {
    const STORED_FLAG: SingletonFlag = SingletonFlag::FrequentAppsHideOpen;
    const STORED_AS_SET: Self = OpenAppListing::Hidden;
    const STORED_AS_CLEARED: Self = OpenAppListing::Listed;
}

impl From<OpenAppListing> for SwitchPosition {
    fn from(open_app_listing: OpenAppListing) -> Self {
        match open_app_listing {
            OpenAppListing::Hidden => SwitchPosition::On,
            OpenAppListing::Listed => SwitchPosition::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryAppOrder {
    ByUsage,
    Alphabetical,
}

impl CategoryAppOrder {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            CategoryAppOrder::ByUsage => CategoryAppOrder::Alphabetical,
            CategoryAppOrder::Alphabetical => CategoryAppOrder::ByUsage,
        }
    }
}

impl SingletonFlagState for CategoryAppOrder {
    const STORED_FLAG: SingletonFlag = SingletonFlag::CategoryAppsSortedByUsage;
    const STORED_AS_SET: Self = CategoryAppOrder::ByUsage;
    const STORED_AS_CLEARED: Self = CategoryAppOrder::Alphabetical;
}

impl From<CategoryAppOrder> for SwitchPosition {
    fn from(category_app_order: CategoryAppOrder) -> Self {
        match category_app_order {
            CategoryAppOrder::ByUsage => SwitchPosition::On,
            CategoryAppOrder::Alphabetical => SwitchPosition::Off,
        }
    }
}

fn hide_open_apps_toggle(open_app_listing: OpenAppListing) -> IconToggle {
    IconToggle::new(
        "Hide apps already open",
        Phosphor::EyeSlash(Weight::Regular),
        open_app_listing,
        SwitchPresses {
            turning_on: "leave out frequent apps with a window on this workspace",
            turning_off: "list frequent apps even with a window on this workspace",
        },
    )
}

fn sort_by_usage_toggle(category_app_order: CategoryAppOrder) -> IconToggle {
    IconToggle::new(
        "Sort categories by usage",
        Phosphor::SortAscending(Weight::Regular),
        category_app_order,
        SwitchPresses {
            turning_on: "list the most launched apps first in each category",
            turning_off: "list the apps in each category alphabetically",
        },
    )
}

impl AppLauncherMenu {
    #[must_use]
    pub fn settings_content(&self) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::LauncherSettings,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::LauncherSettings.title(),
                Phosphor::SquaresFour(Weight::Regular).into(),
                IndicatorState::Idle,
            ),
            &self.settings_expansion,
            AppLauncherMessage::SettingsExpansion,
            IndicatorListing::Listed(vec![
                GroupIndicator::toggle(
                    hide_open_apps_toggle(self.open_app_listing),
                    AppLauncherMessage::ToggleHideOpenApps,
                ),
                GroupIndicator::toggle(
                    sort_by_usage_toggle(self.category_app_order),
                    AppLauncherMessage::ToggleSortByUsage,
                ),
            ]),
        )
    }
}
