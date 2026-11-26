use super::IndicatorGroupInteraction;
use crate::panel_menus::agenda_menu::{
    clock_group::ClockInteraction, location_group::LocationInteraction,
    notification_group::NotificationGroupMessage, quiet_schedule_group::QuietScheduleMessage,
    session_focus_group::SessionFocusMessage, weather_group::WeatherInteraction,
};

impl From<LocationInteraction> for IndicatorGroupInteraction {
    fn from(interaction: LocationInteraction) -> Self {
        IndicatorGroupInteraction::Location(interaction)
    }
}

impl From<WeatherInteraction> for IndicatorGroupInteraction {
    fn from(interaction: WeatherInteraction) -> Self {
        IndicatorGroupInteraction::Weather(interaction)
    }
}

impl From<ClockInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ClockInteraction) -> Self {
        IndicatorGroupInteraction::Clock(interaction)
    }
}

impl From<NotificationGroupMessage> for IndicatorGroupInteraction {
    fn from(interaction: NotificationGroupMessage) -> Self {
        IndicatorGroupInteraction::NotificationGroup(interaction)
    }
}

impl From<QuietScheduleMessage> for IndicatorGroupInteraction {
    fn from(interaction: QuietScheduleMessage) -> Self {
        IndicatorGroupInteraction::QuietSchedule(interaction)
    }
}

impl From<SessionFocusMessage> for IndicatorGroupInteraction {
    fn from(interaction: SessionFocusMessage) -> Self {
        IndicatorGroupInteraction::SessionFocus(interaction)
    }
}
