use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::Placement;
use sottomano_widgets::indicator_groups::{
    fixed_panel_group::FixedPanelGroup, indicator_group_content::IndicatorGroupContent,
    notification_group::NotificationStatus,
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{FixedPanelEntry, StandaloneIndicatorGroup},
};

impl DesktopShell {
    pub(crate) fn fixed_panel_group(&self, entry: FixedPanelEntry) -> FixedPanelGroup<'_> {
        match entry {
            FixedPanelEntry::SessionPower => {
                self.indicator_groups.session_power_group.group_content()
            }
        }
    }

    pub(crate) fn standalone_group(
        &self,
        group: StandaloneIndicatorGroup,
    ) -> IndicatorGroupContent<'_> {
        let config = &self.config;
        let window_placement = Placement::derive(&self.compositor);
        match group {
            StandaloneIndicatorGroup::Clock => self
                .indicator_groups
                .clock_group
                .group_content(self.clock, self.config.calendar.locale),
            StandaloneIndicatorGroup::Notifications => {
                self.indicator_groups.notification_group.group_content(
                    self.notification_status(),
                    &self.indicator_groups.quiet_schedule_group,
                )
            }
            StandaloneIndicatorGroup::Tray => self.indicator_groups.tray_group.group_content(
                config,
                &self.icon_catalog,
                &|owner| window_placement.window_owned_by(owner).is_some(),
            ),
        }
    }

    pub(crate) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet_phase: self.indicator_groups.quiet_schedule_group.phase,
            live: self.notifications.live.len(),
            held: self.notifications.held.len(),
        }
    }
}
