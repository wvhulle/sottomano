use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::Placement;
use sottomano_widgets::widget_groups::{
    fixed_panel_group::FixedPanelGroup, notification_group::NotificationStatus,
    widget_group_content::WidgetGroupContent,
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{FixedPanelEntry, StandaloneWidgetGroup},
    shell_message::ShellInteraction,
};

impl DesktopShell {
    pub(crate) fn fixed_panel_group(
        &self,
        entry: FixedPanelEntry,
    ) -> FixedPanelGroup<'_, ShellInteraction> {
        match entry {
            FixedPanelEntry::SessionPower => self
                .widget_groups
                .session_power_group
                .group_content()
                .map(ShellInteraction::Session),
        }
    }

    pub(crate) fn standalone_group(
        &self,
        group: StandaloneWidgetGroup,
    ) -> WidgetGroupContent<'_, ShellInteraction> {
        let config = &self.config;
        let window_placement = Placement::derive(&self.compositor);
        match group {
            StandaloneWidgetGroup::Clock => self
                .widget_groups
                .clock_group
                .group_content()
                .map(ShellInteraction::Clock),
            StandaloneWidgetGroup::Notifications => self
                .widget_groups
                .notification_group
                .group_content(self.notification_status())
                .map(ShellInteraction::NotificationGroup),
            StandaloneWidgetGroup::Tray => self
                .widget_groups
                .tray_group
                .group_content(config, &|owner| {
                    window_placement.window_owned_by(owner).is_some()
                })
                .map(ShellInteraction::Tray),
        }
    }

    pub(crate) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet_phase: self.widget_groups.quiet_schedule_group.quiet_phase(),
            live: self.notifications.live.len(),
            held: self.notifications.held.len(),
        }
    }
}
