use chrono::Locale;
use sottomano_desktop_sources::desktop_notifications::notification_data::ActionTint;

use super::BatteryTime;
use crate::{
    group_indicators::indicator_group_content::IndicatorListing,
    panel_menus::hardware_menu::power_group::{
        PowerGroup, PowerInteraction, battery_refresh,
        power_profile_indicator::power_profile_control::PowerProfileInteraction,
    },
    widgets::ongoing_toast::OngoingToastActionButton,
};

impl PowerGroup {
    pub(in crate::panel_menus::hardware_menu::power_group) fn battery_time_listing(
        &self,
        locale: Locale,
    ) -> IndicatorListing<'_> {
        let switch_to_saving_profiles =
            self.power_profile
                .profiles_saving_more_power()
                .map(|profile| OngoingToastActionButton {
                    button_label: profile.title().to_owned(),
                    tint: ActionTint::Proceed,
                    on_button_pressed: PowerInteraction::PowerProfile(
                        PowerProfileInteraction::SwitchToProfile(profile),
                    ),
                });
        IndicatorListing::Listed(
            self.battery_time()
                .map(|time| {
                    time.indicator(locale)
                        .ongoing_toast_action_buttons(switch_to_saving_profiles)
                        .with_hover_card(Box::new(move |_| {
                            self.battery.hover_card_with(time.hover_card())
                        }))
                        .refreshed_by(battery_refresh())
                })
                .into_iter()
                .collect(),
        )
    }

    fn battery_time(&self) -> Option<BatteryTime> {
        self.time_indicator
            .time_for(self.battery.get()?, self.power_profile.active_estimate())
    }
}
