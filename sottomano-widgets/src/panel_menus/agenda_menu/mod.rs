pub mod caldav_group;
pub mod clock_group;
pub mod location_group;
pub mod notification_group;
pub mod quiet_schedule_group;
pub mod session_focus_group;
pub mod weather_group;

use sottomano_toml_config::{Config, SecretPaths};

use self::{
    caldav_group::CaldavGroup,
    clock_group::ClockGroup,
    location_group::LocationGroup,
    notification_group::{NotificationGroup, pending_notifications_indicator::NotificationStatus},
    quiet_schedule_group::QuietScheduleGroup,
    session_focus_group::SessionFocusGroup,
    weather_group::WeatherGroup,
};
use crate::{clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent};

pub struct AgendaMenu {
    pub clock_group: ClockGroup,
    pub caldav_group: CaldavGroup,
    pub weather_group: WeatherGroup,
    pub location_group: LocationGroup,
    pub quiet_schedule_group: QuietScheduleGroup,
    pub notification_group: NotificationGroup,
    pub session_focus_group: SessionFocusGroup,
}

impl AgendaMenu {
    pub fn collapse(&mut self) {
        self.clock_group.collapse();
        self.caldav_group.collapse();
        self.weather_group.collapse();
        self.location_group.collapse();
        self.quiet_schedule_group.collapse();
        self.notification_group.collapse();
        self.session_focus_group.collapse();
    }

    #[must_use]
    pub fn new(config: &Config, secrets: &SecretPaths, clock: Clock) -> Self {
        Self {
            clock_group: ClockGroup::default(),
            caldav_group: CaldavGroup::new(secrets),
            weather_group: WeatherGroup::default(),
            location_group: LocationGroup::default(),
            quiet_schedule_group: QuietScheduleGroup::default(),
            notification_group: NotificationGroup::default(),
            session_focus_group: SessionFocusGroup::started_at(
                &config.panel_menus.agenda.session_focus,
                clock.utc(),
            ),
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        notification_status: NotificationStatus,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let locale = config.calendar.locale;
        vec![
            self.clock_group.group_content(clock, locale),
            self.caldav_group
                .group_content(&config.panel_menus.agenda.caldav, clock, locale),
            self.weather_group.group_content(config, clock),
            self.location_group.group_content(),
            self.quiet_schedule_group.group_content(config),
            self.notification_group
                .group_content(notification_status, &self.quiet_schedule_group),
            self.session_focus_group.group_content(config, clock),
        ]
    }
}
