use corredo::{
    popovers::{MenuEntry, MenuItem, MenuRow, MenuRowContent},
    theming::text_color,
    value_controls::SwitchResponse,
};
use sottomano_toml_config::Config;

use super::{AppLauncherMessage, AppLauncherSubmenu};
use crate::{
    group_indicators::{
        indicator_toggle_switch::indicator_toggle_switch,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    indicator_groups::{
        indicator_group_content::GroupEvent,
        sqlite_singleton_flag::{SingletonFlag, SingletonFlagState},
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
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

fn option_entry<'a>(
    toggle: IconToggle,
    message: AppLauncherMessage,
    config: &Config,
) -> MenuEntry<'a, AppLauncherSubmenu, GroupEvent> {
    let content = MenuRowContent::new(toggle.name)
        .icon(IconGlyph::from(toggle.icon).view(config.theme.font.icon_size_px, text_color))
        .value(indicator_toggle_switch(
            config,
            toggle.switch_position,
            SwitchResponse::Pressable,
            1.0,
        ));
    MenuItem::new(MenuRow::new(content).on_press(GroupEvent::from(message))).into()
}

pub(super) fn option_entries<'a>(
    open_app_listing: OpenAppListing,
    category_app_order: CategoryAppOrder,
    config: &Config,
) -> [MenuEntry<'a, AppLauncherSubmenu, GroupEvent>; 2] {
    [
        option_entry(
            hide_open_apps_toggle(open_app_listing),
            AppLauncherMessage::ToggleHideOpenApps,
            config,
        ),
        option_entry(
            sort_by_usage_toggle(category_app_order),
            AppLauncherMessage::ToggleSortByUsage,
            config,
        ),
    ]
}
