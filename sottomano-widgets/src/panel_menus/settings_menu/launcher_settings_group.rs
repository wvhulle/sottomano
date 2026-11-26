use crate::{
    cascading_menus::app_launcher_menu::{
        AppLauncherMenu, AppLauncherMessage,
        launcher_options::{CategoryAppOrder, OpenAppListing},
    },
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupFamily,
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPresses},
    },
};

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

#[must_use]
pub fn launcher_settings_group(app_launcher_menu: &AppLauncherMenu) -> IndicatorGroupContent<'_> {
    IndicatorGroupContent::expandable(
        IndicatorGroupFamily::LauncherSettings,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::LauncherSettings.title(),
            Phosphor::SquaresFour(Weight::Regular).into(),
            IndicatorState::Idle,
        ),
        &app_launcher_menu.settings_expansion,
        AppLauncherMessage::SettingsExpansion,
        IndicatorListing::Listed(vec![
            GroupIndicator::toggle(
                hide_open_apps_toggle(app_launcher_menu.open_app_listing),
                AppLauncherMessage::ToggleHideOpenApps,
            ),
            GroupIndicator::toggle(
                sort_by_usage_toggle(app_launcher_menu.category_app_order),
                AppLauncherMessage::ToggleSortByUsage,
            ),
        ]),
    )
}
